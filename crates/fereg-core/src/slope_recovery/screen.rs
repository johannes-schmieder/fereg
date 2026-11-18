//! Necessary, scale-invariant tests for the exact pairwise recovery search.
use super::basis::WEAK_RATIO;
use crate::error::Result;
use crate::interrupt::row_checkpoint;

const BLOCK: usize = 512;
const PROBES: usize = 12;

#[derive(Default, Clone, Copy)]
pub(super) struct Sketch {
    pub(super) magnitude: f64,
    pub(super) minimum: f64,
    pub(super) maximum: f64,
    pub(super) extrema_rows: [Option<usize>; 2],
}

// Monotone integer keys allow the independent block reductions to vectorize.
// This total-order transform is its own inverse (the sign bit stays fixed).
fn signed_key(bits: i64) -> i64 {
    bits ^ (((bits >> 63) as u64 >> 1) as i64)
}

impl Sketch {
    pub(super) fn new(values: &[f64]) -> Result<Self> {
        let mut result = Self::default();
        for (block, chunk) in values.chunks(BLOCK).enumerate() {
            let first = block * BLOCK;
            row_checkpoint(first)?;
            let (mut low, mut high) = (i64::MAX, i64::MIN);
            for &value in chunk {
                let key = signed_key(value.to_bits() as i64);
                low = low.min(key);
                high = high.max(key);
            }
            let low = f64::from_bits(signed_key(low) as u64);
            let high = f64::from_bits(signed_key(high) as u64);
            if !low.is_finite() || !high.is_finite() {
                result.magnitude = f64::INFINITY;
                result.extrema_rows = [None; 2];
                return Ok(result);
            }
            for (side, local) in [low, high].into_iter().enumerate() {
                let (more_extreme, best) = if side == 0 {
                    (local < result.minimum, &mut result.minimum)
                } else {
                    (local > result.maximum, &mut result.maximum)
                };
                if block == 0 || more_extreme {
                    *best = local;
                    let mut hits = chunk.iter().enumerate().filter(|(_, x)| **x == local);
                    result.extrema_rows[side] = hits.next().map(|(i, _)| first + i);
                    if hits.next().is_some() {
                        result.extrema_rows[side] = None;
                    }
                } else if local == *best {
                    result.extrema_rows[side] = None;
                }
            }
        }
        result.magnitude = result.minimum.abs().max(result.maximum.abs());
        Ok(result)
    }
}

// The anchor ratio fits within δ*max|z|, and simplifying its coefficient
// can add at most another 4δ*max|z|. Thus |a|*max|x|/max|z| is within
// 5δ of one. For a certified centered pair, |a|*range(x)/range(z) is
// within δ of one as well. The relative spreads must agree within 8δ
// (allowing for the two rounded ratios). The full check handles extremes.
fn compatible_spreads(x: Sketch, z: Sketch) -> bool {
    let dx = x.maximum - x.minimum;
    let dz = z.maximum - z.minimum;
    if dx == 0. || dz == 0. {
        return false;
    }
    if !dx.is_finite() || !dz.is_finite() || !(WEAK_RATIO * dz).is_normal() {
        return true;
    }
    let u = dx / x.magnitude;
    let v = dz / z.magnitude;
    (u - v).abs() <= 8. * WEAK_RATIO * u.max(v)
}

/// If the exact search accepts a reference ratio a, its fused residuals are
/// at most δ*Z for Z=max|z|, X=max|x|. The reverse triangle inequality gives
/// ||a|X-Z| <= δ*Z too. Thus at every row |z/Z - sign(a)*x/X| <= 2δ
/// up to arithmetic rounding. Normalizing separately avoids overflow in Z/X.
/// Only signed extrema and uniquely located extrema can affect budget admission.
pub(super) struct PairScreen {
    x_scale: f64,
    z_scale: f64,
    tolerance: f64,
    positive: bool,
    negative: bool,
}

impl PairScreen {
    pub(super) fn new(x: Sketch, z: Sketch, contains_constant: bool) -> Option<Self> {
        let (xs, zs) = (x.magnitude, z.magnitude);
        if !xs.is_finite()
            || !zs.is_finite()
            || xs == 0.
            || zs * WEAK_RATIO <= 8. * f64::from_bits(1)
            || (x.minimum == x.maximum && z.minimum == z.maximum)
            || (contains_constant && !compatible_spreads(x, z))
            || zs / xs == 0.
            || ((zs * (1. - 4. * WEAK_RATIO)) / xs).is_infinite()
        {
            return None;
        }
        // Allow division/subtraction rounding on [-1,1] and absolute
        // subnormal rounding in both the original FMA and its cutoff.
        let tolerance = 2. * WEAK_RATIO + 8. * f64::EPSILON + 4. * (f64::from_bits(1) / zs);
        let (xl, xh) = (x.minimum / xs, x.maximum / xs);
        let (zl, zh) = (z.minimum / zs, z.maximum / zs);
        let positive = (zl - xl).abs().max((zh - xh).abs()) <= tolerance;
        let negative = (zl + xh).abs().max((zh + xl).abs()) <= tolerance;
        (positive || negative).then_some(Self {
            x_scale: xs,
            z_scale: zs,
            tolerance,
            positive,
            negative,
        })
    }

    pub(super) fn accepts_row(&mut self, x: f64, z: f64) -> bool {
        let (u, v) = (x / self.x_scale, z / self.z_scale);
        self.positive &= (v - u).abs() <= self.tolerance;
        self.negative &= (v + u).abs() <= self.tolerance;
        self.positive || self.negative
    }

    /// Positional probes save a full scan after a pair has been charged.
    /// Their locations may change under permutation; their result cannot
    /// change which later pairs fit the budget or reject a valid change.
    pub(super) fn accepts_positions(&mut self, x: &[f64], z: &[f64]) -> bool {
        let last = x.len() - 1;
        (0..PROBES).all(|i| {
            let row = (last / (PROBES - 1)) * i + ((last % (PROBES - 1)) * i) / (PROBES - 1);
            self.accepts_row(x[row], z[row])
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tie_across_blocks_never_selects_a_row() {
        let mut values = vec![1.; BLOCK + 5];
        values[BLOCK - 1] = 3.;
        values[BLOCK] = 3.;
        values[BLOCK + 1] = -4.;
        for _ in 0..2 {
            let s = Sketch::new(&values).unwrap();
            assert_eq!((s.minimum, s.maximum, s.magnitude), (-4., 3., 4.));
            assert!(s.extrema_rows[0].is_some());
            assert!(s.extrema_rows[1].is_none());
            values.reverse();
        }
        values[BLOCK - 1] = -2.;
        assert_eq!(
            Sketch::new(&values).unwrap().extrema_rows,
            [Some(BLOCK + 1), Some(BLOCK)]
        );
        values.reverse();
        assert_eq!(
            Sketch::new(&values).unwrap().extrema_rows,
            [Some(3), Some(4)]
        );
    }

    #[test]
    fn both_orientations_of_a_weak_pair_survive_all_witnesses() {
        let xs = [1., -1., 3., -3., 0.01];
        for coefficient in [-2_f64.powi(930), 2_f64.powi(-930)] {
            let zs = std::array::from_fn::<_, 5, _>(|i| {
                coefficient * (xs[i] + (i as f64) * 2_f64.powi(-20))
            });
            for (x, z) in [(&xs, &zs), (&zs, &xs)] {
                let (sx, sz) = (Sketch::new(x).unwrap(), Sketch::new(z).unwrap());
                for reverse in [false, true] {
                    let mut s = PairScreen::new(sx, sz, false).unwrap();
                    for i in 0..x.len() {
                        let j = if reverse { x.len() - 1 - i } else { i };
                        assert!(s.accepts_row(x[j], z[j]));
                    }
                    assert!(s.accepts_positions(x, z));
                }
            }
        }
    }

    #[test]
    fn impossible_ranges_and_unrepresentable_slopes_do_not_enter_the_budget() {
        let x = Sketch::new(&[-2., 2.]).unwrap();
        let z = Sketch::new(&[1., 2.]).unwrap();
        assert!(PairScreen::new(x, z, false).is_none());
        let tiny = Sketch::new(&[0., f64::MIN_POSITIVE]).unwrap();
        let huge = Sketch::new(&[0., f64::MAX]).unwrap();
        assert!(PairScreen::new(tiny, huge, false).is_none());
        assert!(PairScreen::new(huge, tiny, false).is_none());
    }

    #[test]
    fn relative_spreads_rule_out_independent_translations_before_charging() {
        let x = Sketch::new(&[2_f64.powi(24), 2_f64.powi(24) + 2.]).unwrap();
        let z = Sketch::new(&[-2_f64.powi(22), 1. - 2_f64.powi(22)]).unwrap();
        let near = Sketch::new(&[2_f64.powi(24), 2_f64.powi(24) + 2. + 2_f64.powi(-18)]).unwrap();
        for (a, b) in [(x, z), (z, x)] {
            assert!(PairScreen::new(a, b, false).is_some());
            assert!(PairScreen::new(a, b, true).is_none());
        }
        assert!(PairScreen::new(x, near, true).is_some());
    }

    #[test]
    fn a_positional_disagreement_cannot_affect_admission() {
        let mut x = std::array::from_fn::<_, 25, _>(|i| if i % 2 == 0 { -1. } else { 1. });
        let mut z = x;
        z[3] = -1.;
        for positional_hit in [false, true] {
            let sx = Sketch::new(&x).unwrap();
            let sz = Sketch::new(&z).unwrap();
            assert_eq!((sx.extrema_rows, sz.extrema_rows), ([None; 2], [None; 2]));
            // Both permutations are admitted and charged. Only the cheaper
            // post-admission route can see the misplaced row in its probes.
            let mut screen = PairScreen::new(sx, sz, false).unwrap();
            assert_eq!(screen.accepts_positions(&x, &z), !positional_hit);
            x.swap(0, 3);
            z.swap(0, 3);
        }
    }
}
