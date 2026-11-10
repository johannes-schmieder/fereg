use super::hypot;
use std::cmp::Ordering;

/// splitmix64: a fixed integer stream, identical on every platform.
struct Stream(u64);

impl Stream {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// `[1, 2)` with a uniformly random 52-bit fraction.
    fn significand(&mut self) -> f64 {
        f64::from_bits((self.next() >> 12) | (1023 << 52))
    }

    fn below(&mut self, n: u64) -> i32 {
        (self.next() % n) as i32
    }

    /// `m · 2^e` with a random significand `m`, a random exponent `e ∈ [low, low + span)` and a random sign.
    fn signed(&mut self, low: i32, span: u64) -> f64 {
        let m = self.significand();
        let e = low + self.below(span);
        let x = scale(m, e);
        if self.next() & 1 == 1 {
            -x
        } else {
            x
        }
    }
}

/// `x · 2^k`, exact while the result is normal (rounded once into the subnormal range).
fn scale(x: f64, k: i32) -> f64 {
    let k = k.clamp(-2200, 2200);
    let step = |e: i32| super::pow2(e.clamp(-1022, 1023));
    let (k1, k2) = (k / 2, k - k / 2);
    x * step(k1) * step(k2)
}

/// A positive finite `x` as `m · 2^e` exactly.
fn decompose(x: f64) -> (u64, i32) {
    let bits = x.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1 << 52) - 1);
    if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), biased - 1075)
    }
}

/// Little-endian 256-bit unsigned integer: enough for every comparison below.
#[derive(Clone, Copy, PartialEq, Eq)]
struct U256([u64; 4]);

impl U256 {
    fn shifted(value: u128, shift: u32) -> Option<Self> {
        if value != 0 && 128 - value.leading_zeros() + shift > 256 {
            return None;
        }
        let mut limbs = [0_u64; 4];
        let (words, bits) = ((shift / 64) as usize, shift % 64);
        let parts = [value as u64, (value >> 64) as u64, 0];
        for (index, &part) in parts.iter().enumerate() {
            let low = part << bits;
            let high = if bits == 0 { 0 } else { part >> (64 - bits) };
            if index + words < 4 {
                limbs[index + words] |= low;
            }
            if index + words + 1 < 4 {
                limbs[index + words + 1] |= high;
            }
        }
        Some(Self(limbs))
    }

    fn add(self, other: Self) -> Option<Self> {
        let mut limbs = [0_u64; 4];
        let mut carry = 0_u64;
        for (index, limb) in limbs.iter_mut().enumerate() {
            let (sum, first) = self.0[index].overflowing_add(other.0[index]);
            let (sum, second) = sum.overflowing_add(carry);
            *limb = sum;
            carry = u64::from(first || second);
        }
        (carry == 0).then_some(Self(limbs))
    }
}

impl Ord for U256 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.iter().rev().cmp(other.0.iter().rev())
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A nonnegative dyadic rational `m · 2^e`.
#[derive(Clone, Copy)]
struct Dyadic {
    m: u128,
    e: i32,
}

impl Dyadic {
    fn of(x: f64) -> Self {
        let (m, e) = decompose(x);
        Self {
            m: u128::from(m),
            e,
        }
    }

    /// `(u + v) / 2` exactly.
    fn midpoint(u: Self, v: Self) -> Self {
        let e = u.e.min(v.e);
        Self {
            m: (u.m << (u.e - e)) + (v.m << (v.e - e)),
            e: e - 1,
        }
    }
}

/// The rounding boundary between `x` and the next double up, `None` above `+∞`.
fn boundary_above(x: f64) -> Option<Dyadic> {
    if x == f64::INFINITY {
        return None;
    }
    if x == f64::MAX {
        // Values from MAX + ulp/2 round to +∞.
        return Some(Dyadic {
            m: (1 << 54) - 1,
            e: 970,
        });
    }
    Some(Dyadic::midpoint(
        Dyadic::of(x),
        Dyadic::of(f64::from_bits(x.to_bits() + 1)),
    ))
}

/// The rounding boundary between `x` and the next double down, `None` at zero.
fn boundary_below(x: f64) -> Option<Dyadic> {
    if x == 0.0 {
        return None;
    }
    if x == f64::INFINITY {
        return boundary_above(f64::MAX);
    }
    let previous = f64::from_bits(x.to_bits() - 1);
    Some(if previous == 0.0 {
        Dyadic { m: 1, e: -1075 }
    } else {
        Dyadic::midpoint(Dyadic::of(previous), Dyadic::of(x))
    })
}

/// `x² + y²` compared with `bound²`, exactly; `None` when the shifts leave 256 bits.
fn compare_sum_of_squares(x: f64, y: f64, bound: Dyadic) -> Option<Ordering> {
    let (mx, ex) = decompose(x);
    let (my, ey) = decompose(y);
    let base = (2 * ex).min(2 * ey).min(2 * bound.e);
    let term = |m: u128, e: i32| U256::shifted(m * m, (e - base) as u32);
    let sum = term(u128::from(mx), 2 * ex)?.add(term(u128::from(my), 2 * ey)?)?;
    let square = term(bound.m, 2 * bound.e)?;
    Some(sum.cmp(&square))
}

/// The result's distance from correct rounding of `√(x² + y²)`: 0 (correctly
/// rounded), 1 (a neighbour of it) or 2 (further), decided exactly.
/// `None` when `y ≤ 2^-28 x` is too small to compare in 256 bits.
fn ulps_from_correct_rounding(x: f64, y: f64, result: f64) -> Option<u32> {
    let (x, y) = if x.abs() < y.abs() {
        (y.abs(), x.abs())
    } else {
        (x.abs(), y.abs())
    };
    let within = |low: Option<Dyadic>, high: Option<Dyadic>| -> Option<bool> {
        let above_low = match low {
            Some(low) => compare_sum_of_squares(x, y, low)? != Ordering::Less,
            None => true,
        };
        let below_high = match high {
            Some(high) => compare_sum_of_squares(x, y, high)? != Ordering::Greater,
            None => true,
        };
        Some(above_low && below_high)
    };
    if within(boundary_below(result), boundary_above(result))? {
        return Some(0);
    }
    let down = if result == 0.0 {
        0.0
    } else {
        f64::from_bits(result.to_bits() - 1)
    };
    let up = if result == f64::INFINITY {
        result
    } else {
        f64::from_bits(result.to_bits() + 1)
    };
    let low = if result == 0.0 {
        None
    } else {
        boundary_below(down)
    };
    let high = if result == f64::INFINITY {
        None
    } else {
        boundary_above(up)
    };
    Some(if within(low, high)? { 1 } else { 2 })
}

/// Distance from correct rounding, also where the smaller argument is negligible.
fn distance(x: f64, y: f64) -> u32 {
    let result = hypot(x, y);
    assert!(
        result >= x.abs().max(y.abs()),
        "hypot({x:e}, {y:e}) = {result:e}"
    );
    ulps_from_correct_rounding(x, y, result).unwrap_or_else(|| {
        // Outside 256 bits the smaller magnitude is below 2^-60 of the larger,
        // whose own value is then the correctly rounded result.
        let (large, small) = (x.abs().max(y.abs()), x.abs().min(y.abs()));
        assert!(small <= large * super::pow2(-28), "{x:e}, {y:e}");
        u32::from(result != large)
    })
}

/// Random and adversarial pairs, all finite and positive.
fn checked_pairs() -> Vec<(f64, f64)> {
    let mut stream = Stream(0x5eed_0f4e_9a7c_0001);
    let mut pairs = Vec::new();
    // Uniform significands, exponent gaps 0..=64, base exponents over the whole range.
    for _ in 0..120_000 {
        let base = stream.below(2046) - 1022;
        let gap = stream.below(65);
        pairs.push((
            scale(stream.significand(), base),
            scale(stream.significand(), base - gap),
        ));
    }
    // The shortcut's boundary: gaps 27..=30.
    for _ in 0..20_000 {
        let base = stream.below(200) - 100;
        let gap = 27 + stream.below(4);
        pairs.push((
            scale(stream.significand(), base),
            scale(stream.significand(), base - gap),
        ));
    }
    // Nearly equal arguments, a few ulps apart.
    for _ in 0..20_000 {
        let a = scale(stream.significand(), stream.below(200) - 100);
        let b = f64::from_bits(a.to_bits() - stream.next() % 9);
        pairs.push((a, b));
    }
    // Results at binade boundaries: a just below 2^k and b ≈ 2^k √(2δ).
    for _ in 0..20_000 {
        let k = stream.below(200) - 100;
        let a = f64::from_bits(scale(1.0, k).to_bits() - 1 - stream.next() % 1000);
        let delta = 1.0 - a / scale(1.0, k);
        let b = scale((2.0 * delta).sqrt(), k) * (1.0 + (stream.significand() - 1.5) * 1e-3);
        pairs.push((a, b));
    }
    // Subnormal and mixed subnormal/normal pairs.
    for _ in 0..20_000 {
        let a = f64::from_bits(stream.next() % (1 << 53));
        let b = f64::from_bits(stream.next() % (1 << 52));
        if a > 0.0 && b > 0.0 {
            pairs.push((a, b));
        }
    }
    // Near overflow, including results that round to +∞.
    for _ in 0..10_000 {
        let a = f64::from_bits(f64::MAX.to_bits() - stream.next() % (1 << 50));
        let b = scale(stream.significand(), 1023 - stream.below(40));
        pairs.push((a, b));
    }
    // Exact Pythagorean triples, scaled.
    for (p, q) in [
        (3.0, 4.0),
        (5.0, 12.0),
        (8.0, 15.0),
        (20.0, 21.0),
        (119.0, 120.0),
    ] {
        for k in (-1070..1020).step_by(7) {
            pairs.push((scale(p, k), scale(q, k)));
        }
    }
    pairs.extend([
        (f64::MAX, f64::MAX),
        (f64::MAX, 1.0),
        (f64::MIN_POSITIVE, f64::MIN_POSITIVE),
        (f64::from_bits(1), f64::from_bits(1)),
        (f64::from_bits(1), f64::MIN_POSITIVE),
        (1.0, f64::EPSILON),
        (1.0, 1.0),
    ]);
    pairs
}

#[test]
fn hypot_is_within_one_ulp_of_correct_rounding_by_exact_arithmetic() {
    let pairs = checked_pairs();
    let mut subnormal_neighbours = 0_usize;
    for &(x, y) in &pairs {
        let result = hypot(x, y);
        let distance = distance(x, y);
        assert!(
            distance <= 1,
            "hypot({x:e}, {y:e}) = {result:e} is more than one ulp from correct rounding"
        );
        // One correction step leaves a normal result off by one only within
        // about 2^-100 of a rounding boundary, which no pair here reaches; a
        // subnormal result is rounded a second time.
        if result >= f64::MIN_POSITIVE {
            assert_eq!(distance, 0, "hypot({x:e}, {y:e}) = {result:e}");
        } else {
            subnormal_neighbours += distance as usize;
        }
    }
    assert!(
        subnormal_neighbours > 0,
        "the subnormal family no longer rounds twice"
    );
}

#[test]
fn exact_results_are_exact() {
    for k in -1070..1020 {
        for (p, q, r) in [(3.0, 4.0, 5.0), (5.0, 12.0, 13.0), (65.0, 72.0, 97.0)] {
            let expected = scale(r, k);
            assert_eq!(
                hypot(scale(p, k), scale(q, k)).to_bits(),
                expected.to_bits()
            );
        }
        let power = scale(1.0, k);
        assert_eq!(hypot(power, 0.0).to_bits(), power.to_bits());
    }
}

#[test]
fn special_values_follow_ieee_754() {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    for x in [0.0, 1.0, -3.5, f64::MAX, f64::from_bits(1), nan, inf, -inf] {
        assert_eq!(hypot(inf, x), inf);
        assert_eq!(hypot(x, -inf), inf);
    }
    for x in [0.0, -0.0, 1.0, -2.0e300, f64::from_bits(7)] {
        assert!(hypot(nan, x).is_nan());
        assert!(hypot(x, nan).is_nan());
        assert_eq!(hypot(x, 0.0).to_bits(), x.abs().to_bits());
        assert_eq!(hypot(-0.0, x).to_bits(), x.abs().to_bits());
    }
    assert_eq!(hypot(nan, 1.0).to_bits(), f64::NAN.to_bits());
    assert_eq!(hypot(f64::MAX, f64::MAX), inf);
    assert_eq!(hypot(f64::MAX, 1.0), f64::MAX);
}

#[test]
fn hypot_is_symmetric_in_arguments_and_signs() {
    let mut stream = Stream(17);
    for _ in 0..10_000 {
        let x = stream.signed(-100, 200);
        let y = stream.signed(-100, 200);
        let h = hypot(x, y).to_bits();
        for (u, v) in [(y, x), (-x, y), (x, -y), (-y, -x)] {
            assert_eq!(hypot(u, v).to_bits(), h);
        }
    }
}

/// FNV-1a over the input and result bits of a fixed set of pairs.
fn golden_digest(count: usize) -> u64 {
    let mut stream = Stream(0x00c0_ffee_d16e_57ed);
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    let mut absorb = |x: f64| {
        for byte in x.to_bits().to_le_bytes() {
            digest ^= u64::from(byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for index in 0..count {
        let (x, y) = match index % 8 {
            // Arbitrary finite bit patterns: subnormals, signs, wide gaps.
            0 => {
                let pattern = |s: &mut Stream| loop {
                    let x = f64::from_bits(s.next());
                    if x.is_finite() {
                        break x;
                    }
                };
                (pattern(&mut stream), pattern(&mut stream))
            }
            // Nearly equal magnitudes.
            1 => {
                let a = stream.signed(-1022, 2046);
                (a, f64::from_bits(a.to_bits() ^ (stream.next() % 16)))
            }
            // Exponents within ±20 of 1, as the accumulated norms meet them.
            2..=4 => (stream.signed(-20, 40), stream.signed(-20, 40)),
            // Gaps 0..=40 over the whole exponent range.
            _ => {
                let base = stream.below(2046) - 1022;
                let gap = stream.below(41);
                (stream.signed(base, 1), stream.signed(base - gap, 1))
            }
        };
        absorb(x);
        absorb(y);
        absorb(hypot(x, y));
    }
    digest
}

/// The same pairs give the same bits on every platform; a change here is an
/// arithmetic change of every caller (fedemean's LSMR, fereg's QR).
#[test]
fn hypot_bits_match_the_cross_platform_golden_digest() {
    assert_eq!(golden_digest(1_000_000), GOLDEN_DIGEST);
}

const GOLDEN_DIGEST: u64 = 0x128efb568e0c0b75;
