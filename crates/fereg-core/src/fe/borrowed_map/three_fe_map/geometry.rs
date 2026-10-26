use super::super::*;
use crate::fe::BorrowedInterceptEffect;

/// Exact-arithmetic moment bounds for four symmetric sweeps. Every pair
/// removes the same global constant; extra common directions remain.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Geometry {
    bounds: [f64; 4],
}

impl Geometry {
    pub(crate) fn inspect(
        design: &BorrowedInterceptFeDesign<'_>,
        budget: Option<FeSolverMemoryBudget>,
    ) -> Result<(Option<Self>, SolverWorkspacePeak)> {
        let effects = design.effects();
        if effects.len() != 3 || design.nobs() == 0 {
            return Ok((None, SolverWorkspacePeak::ZERO));
        }
        let size = [(0, 1), (0, 2), (1, 2)]
            .into_iter()
            .map(|(i, j)| effects[i].n_levels().min(effects[j].n_levels()))
            .max()
            .unwrap_or(0);
        // Exact integer counts make every lane split produce identical bounds.
        // Extra lanes are optional; a refused or unaffordable split keeps the
        // single-lane admission and its refusal semantics.
        let mut lanes = crate::parallel::StagePlan::current(design.nobs() as u128, design.nobs())
            .lanes
            .min(SPLIT_LANE_LIMIT);
        let mut counts = Vec::<u64>::new();
        while lanes > 1 {
            let affordable = size
                .checked_mul(lanes)
                .filter(|&cells| budget.is_none_or(|b| b.fits(cells as u128 * 8)));
            if affordable.is_some_and(|cells| counts.try_reserve_exact(cells).is_ok()) {
                break;
            }
            lanes = 1;
        }
        if lanes == 1 {
            let bytes = size as u128 * 8;
            if budget.is_some_and(|b| !b.fits(bytes)) {
                return Ok((None, SolverWorkspacePeak::ZERO));
            }
            if counts.try_reserve_exact(size).is_err() {
                return Ok((None, SolverWorkspacePeak::ZERO));
            }
        }
        let bytes = counts.capacity() as u128 * 8;
        let peak = SolverWorkspacePeak::from_bytes(bytes)?;
        if budget.is_some_and(|b| !b.fits(bytes)) {
            return Ok((None, peak));
        }
        counts.resize(size * lanes, 0);
        let mut squares = [0_u128; 3];
        let mut minimum = [u128::MAX; 3];
        let mut maximum = [0_u128; 3];
        for (i, effect) in effects.iter().enumerate() {
            for level in 0..effect.n_levels() {
                if level % 1024 == 0 {
                    crate::interrupt::checkpoint()?;
                }
                let n = effect.level_index().checked_members(level)?.len() as u128;
                let Some(sum) = n.checked_mul(n).and_then(|v| squares[i].checked_add(v)) else {
                    return Ok((None, peak));
                };
                squares[i] = sum;
                minimum[i] = minimum[i].min(n);
                maximum[i] = maximum[i].max(n);
            }
        }
        let mut coherence = [0.0; 3];
        for (slot, (i, j)) in [(0, 1), (0, 2), (1, 2)].into_iter().enumerate() {
            crate::interrupt::checkpoint()?;
            if marginal_saturation(
                design.nobs() as u128,
                [squares[i], squares[j]],
                [minimum[i], minimum[j]],
                [maximum[i], maximum[j]],
            ) == Some(true)
            {
                coherence[slot] = 1.0;
                continue;
            }
            let Some((count_square, weighted)) =
                pair_moments(effects[i], effects[j], &mut counts, lanes)?
            else {
                return Ok((None, peak));
            };
            let pair = (|| {
                let n = design.nobs() as u128;
                let nn = n.checked_mul(n)?;
                let numerator = nn
                    .checked_mul(count_square)?
                    .checked_add(squares[i].checked_mul(squares[j])?)?
                    .checked_sub(n.checked_mul(2)?.checked_mul(weighted)?)?;
                let denominator = nn.checked_mul(minimum[i])?.checked_mul(minimum[j])?;
                integer_bound(numerator, denominator)
            })();
            let Some(bound) = pair else {
                return Ok((None, peak));
            };
            coherence[slot] = bound;
        }
        Ok((
            Some(Self {
                bounds: propagate(coherence),
            }),
            peak,
        ))
    }

    /// The bound depends only on the design, so the same prediction serves the
    /// main projection and the final residual phase at that phase's target.
    pub(crate) fn map_checkpoint(
        self,
        weights: BorrowedMapWeights<'_>,
        map: MapConfig,
    ) -> Option<usize> {
        if !map.symmetric || !matches!(weights, BorrowedMapWeights::Unit) {
            return None;
        }
        self.predicted_sweeps(crate::fe::auto::direct_lsmr_tolerance(map.tolerance))
    }

    pub(crate) fn predicted_sweeps(self, target: f64) -> Option<usize> {
        self.bounds.iter().position(|&b| b <= target).map(|i| i + 1)
    }
}

/// The lower bound uses integer cells (sum n_ab^2 >= N) and marginal maxima.
/// Prove the old computation cannot overflow before bypassing its row scans.
fn marginal_saturation(
    n: u128,
    squares: [u128; 2],
    minimum: [u128; 2],
    maximum: [u128; 2],
) -> Option<bool> {
    let nn = n.checked_mul(n)?;
    nn.checked_mul(nn)?.checked_mul(2)?;
    let upper_weighted = maximum[0]
        .checked_mul(squares[1])?
        .min(maximum[1].checked_mul(squares[0])?);
    let lower = nn
        .checked_mul(n)?
        .checked_add(squares[0].checked_mul(squares[1])?)?
        .checked_sub(n.checked_mul(2)?.checked_mul(upper_weighted)?)?;
    let denominator = nn.checked_mul(minimum[0])?.checked_mul(minimum[1])?;
    Some(denominator > 0 && lower >= denominator)
}

/// Concurrent pair-moment lanes. Integer sums are exact under any split, so
/// this caps only scratch, never results.
const SPLIT_LANE_LIMIT: usize = 64;

fn pair_moments(
    a: BorrowedInterceptEffect<'_>,
    b: BorrowedInterceptEffect<'_>,
    counts: &mut [u64],
    lanes: usize,
) -> Result<Option<(u128, u128)>> {
    use rayon::prelude::*;
    let (outer, inner) = if a.n_levels() >= b.n_levels() {
        (a, b)
    } else {
        (b, a)
    };
    if lanes <= 1 {
        return pair_moment_range(outer, inner, 0..outer.n_levels(), counts);
    }
    // Contiguous outer-level ranges with balanced membership.
    let offsets = outer.level_index().membership_offsets();
    let total = offsets[outer.n_levels()] - offsets[0];
    let boundary = |lane: usize| {
        let target = offsets[0] + total / lanes * lane + (total % lanes) * lane / lanes;
        offsets[..outer.n_levels()].partition_point(|&offset| offset < target)
    };
    let stride = counts.len() / lanes;
    let partials: Vec<_> = counts
        .par_chunks_mut(stride)
        .enumerate()
        .map(|(lane, counts)| {
            let end = if lane + 1 == lanes {
                outer.n_levels()
            } else {
                boundary(lane + 1)
            };
            pair_moment_range(outer, inner, boundary(lane)..end, counts)
        })
        .collect();
    let mut count_square = 0_u128;
    let mut weighted = 0_u128;
    // The first lane error, in level order, is the serial loop's first error.
    for partial in partials {
        let Some((s, w)) = partial? else {
            return Ok(None);
        };
        let (Some(s), Some(w)) = (count_square.checked_add(s), weighted.checked_add(w)) else {
            return Ok(None);
        };
        count_square = s;
        weighted = w;
    }
    Ok(Some((count_square, weighted)))
}

fn pair_moment_range(
    outer: BorrowedInterceptEffect<'_>,
    inner: BorrowedInterceptEffect<'_>,
    levels: std::ops::Range<usize>,
    counts: &mut [u64],
) -> Result<Option<(u128, u128)>> {
    let mut count_square = 0_u128;
    let mut weighted = 0_u128;
    for level in levels {
        if level % 1024 == 0 {
            crate::interrupt::checkpoint()?;
        }
        let members = outer.level_index().checked_members(level)?;
        for (position, &row) in members.iter().enumerate() {
            if position % 16_384 == 0 {
                crate::interrupt::checkpoint()?;
            }
            let count = &mut counts[inner.codes()[row] as usize];
            let Some(next) = count.checked_add(1) else {
                return Ok(None);
            };
            *count = next;
        }
        // Each occupied cell is accumulated exactly once. All integer terms
        // are nonnegative, so checked success/failure agrees with the old sum.
        for (position, &row) in members.iter().enumerate() {
            if position % 16_384 == 0 {
                crate::interrupt::checkpoint()?;
            }
            let code = inner.codes()[row] as usize;
            let n = std::mem::take(&mut counts[code]) as u128;
            if n == 0 {
                continue;
            }
            let inner_count = inner.level_index().checked_members(code)?.len() as u128;
            let Some((s, w)) = (|| {
                Some((
                    count_square.checked_add(n.checked_mul(n)?)?,
                    weighted.checked_add(
                        n.checked_mul(members.len() as u128)?
                            .checked_mul(inner_count)?,
                    )?,
                ))
            })() else {
                return Ok(None);
            };
            count_square = s;
            weighted = w;
        }
    }
    Ok(Some((count_square, weighted)))
}

fn up(x: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}

fn integer_bound(numerator: u128, denominator: u128) -> Option<f64> {
    if denominator == 0 {
        return None;
    }
    if numerator == 0 {
        return Some(0.0);
    }
    // One adjacent float encloses each nearest-rounded integer conversion.
    // Positive u128 ratios and the four powers below stay in the normal range.
    let lower_denominator = f64::from_bits((denominator as f64).to_bits() - 1);
    Some(up(up(up(numerator as f64) / lower_denominator).sqrt()).min(1.0))
}

fn propagate([a, b, c]: [f64; 3]) -> [f64; 4] {
    let plus = |x, y| up(x + y);
    let times = |x, y| up(x * y);
    let cc = times(c, c);
    let u = plus(plus(a, times(b, c)), times(a, cc));
    let v = plus(b, times(a, c));
    let matrix = [
        [times(a, u), times(a, v)],
        [plus(times(c, cc), times(b, u)), plus(cc, times(b, v))],
    ];
    let mut moment = [1.0, 1.0];
    std::array::from_fn(|_| {
        moment = matrix.map(|row| plus(times(row[0], moment[0]), times(row[1], moment[1])));
        moment[0].max(moment[1])
    })
}

#[cfg(test)]
#[path = "../../../../../fereg-testkit/src/pair_incidence_reference.rs"]
mod reference;

#[cfg(test)]
mod tests {
    #[test]
    fn marginal_saturation_keeps_equality_and_overflow_boundaries() {
        use super::*;
        // Sixteen distinct occupied cells: L=D exactly.
        assert_eq!(
            marginal_saturation(16, [32, 64], [2, 4], [2, 4]),
            Some(true)
        );
        // Full balanced product: the exact coefficient is zero.
        assert_eq!(
            marginal_saturation(16, [64, 64], [4, 4], [4, 4]),
            Some(false)
        );
        assert_eq!(
            marginal_saturation(1_u128 << 40, [1; 2], [1; 2], [1; 2]),
            None
        );
        assert_eq!(
            marginal_saturation(16, [32, 64], [0, 4], [2, 4]),
            Some(false)
        );
    }

    #[test]
    fn complete_geometry_matches_cartesian_witness_in_every_effect_order() {
        use super::*;
        use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for case in 0..16 {
            let rows = 128 + case;
            let codes: [Vec<u32>; 3] = std::array::from_fn(|e| {
                (0..rows)
                    .map(|r| {
                        let code = match case {
                            0 => r / [2, 4, 8][e],
                            1 => (r / [1, 4, 16][e]) % 4,
                            2 => 4 * (r / 64) + (r / [1, 4, 16][e]) % 4,
                            3 => {
                                if e < 2 {
                                    r % 8
                                } else {
                                    (r / 8) % 2
                                }
                            }
                            _ => (r * r * (2 * e + 1) + r * (case + e + 1) + r / (e + 1)) % (e + 3),
                        };
                        code as u32
                    })
                    .collect()
            });
            // Canonicalize arbitrary generated labels to avoid unused levels.
            let codes = codes.map(|mut c| {
                let mut labels = c.clone();
                labels.sort_unstable();
                labels.dedup();
                for v in &mut c {
                    *v = labels.binary_search(v).unwrap() as u32;
                }
                c
            });
            let levels = codes
                .each_ref()
                .map(|c| *c.iter().max().unwrap() as usize + 1);
            let indexes: Vec<_> = codes
                .iter()
                .zip(levels)
                .map(|(c, n)| LevelIndex::from_codes(c, n).unwrap())
                .collect();
            let sources: Vec<_> = codes
                .iter()
                .zip(levels)
                .zip(&indexes)
                .map(|((c, n), i)| ValidatedBorrowedInterceptSource::new(c, n, i).unwrap())
                .collect();
            for order in permutations {
                let design = BorrowedInterceptFeDesign::new(
                    rows,
                    order.map(|e| sources[e].effect()).to_vec(),
                )
                .unwrap();
                let pairs = [(0, 1), (0, 2), (1, 2)].map(|(i, j)| {
                    let (n, d) = reference::centered_ratio(&codes[order[i]], &codes[order[j]]);
                    integer_bound(n, d).unwrap()
                });
                let expected = propagate(pairs);
                let actual = Geometry::inspect(&design, None).unwrap().0.unwrap();
                assert_eq!(
                    actual.bounds.map(f64::to_bits),
                    expected.map(f64::to_bits),
                    "case {case}, order {order:?}"
                );
                for bound in expected {
                    for target in [
                        bound,
                        f64::from_bits(bound.to_bits().saturating_sub(1)),
                        bound * 2.0,
                        1e-14,
                    ] {
                        assert_eq!(
                            actual.predicted_sweeps(target),
                            expected.iter().position(|&v| v <= target).map(|i| i + 1)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn split_pair_moments_and_bounds_are_exact_for_every_lane_count() {
        use super::*;
        use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};
        let rows = 120_011;
        let levels = [37_usize, 211, 1_009];
        let codes: [Vec<u32>; 3] = std::array::from_fn(|e| {
            (0..rows)
                .map(|r| ((r * (2 * e + 3) + r / (e + 5) + (r * r) % 7) % levels[e]) as u32)
                .collect()
        });
        let indexes: Vec<_> = codes
            .iter()
            .zip(levels)
            .map(|(c, n)| LevelIndex::from_codes(c, n).unwrap())
            .collect();
        let sources: Vec<_> = codes
            .iter()
            .zip(levels)
            .zip(&indexes)
            .map(|((c, n), i)| ValidatedBorrowedInterceptSource::new(c, n, i).unwrap())
            .collect();
        let design =
            BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect())
                .unwrap();
        let effects = design.effects();
        for (i, j) in [(0, 1), (0, 2), (1, 2)] {
            let size = levels[i].min(levels[j]);
            let mut serial = vec![0_u64; size];
            let expected = pair_moments(effects[i], effects[j], &mut serial, 1).unwrap();
            assert!(expected.is_some());
            for lanes in [2, 3, 7, 64] {
                let mut counts = vec![0_u64; size * lanes];
                let actual = rayon::ThreadPoolBuilder::new()
                    .num_threads(4)
                    .build()
                    .unwrap()
                    .install(|| pair_moments(effects[i], effects[j], &mut counts, lanes))
                    .unwrap();
                assert_eq!(actual, expected, "pair {i},{j} lanes {lanes}");
                assert!(counts.iter().all(|&count| count == 0));
            }
        }
        let serial = Geometry::inspect(&design, None).unwrap().0.unwrap();
        for threads in [2, 4, 16] {
            let split = crate::ThreadTeam::new(
                crate::ThreadConfig::from_requested(threads, false).unwrap(),
            )
            .install(|| Geometry::inspect(&design, None))
            .unwrap()
            .unwrap()
            .0
            .unwrap();
            assert_eq!(
                split.bounds.map(f64::to_bits),
                serial.bounds.map(f64::to_bits)
            );
        }
    }

    #[test]
    fn integer_and_sweep_bounds_round_outward_at_eligibility_boundary() {
        use super::*;
        assert_eq!(integer_bound(0, 1), Some(0.));
        assert_eq!(integer_bound(1, 0), None);
        assert!(integer_bound(1, 4).unwrap() > 0.5);
        assert!(integer_bound((1_u128 << 100) + 1, 1_u128 << 102).unwrap() > 0.5);
        assert_eq!(integer_bound(u128::MAX, 1), Some(1.));
        assert_eq!(propagate([0.; 3]), [0.; 4]);
        let bounds = propagate([0.01; 3]);
        let g = Geometry { bounds };
        assert_eq!(g.predicted_sweeps(bounds[3]), Some(4));
        assert_eq!(
            g.predicted_sweeps(f64::from_bits(bounds[3].to_bits() - 1)),
            None
        );
        assert!(propagate([1.; 3]).iter().all(|&b| b > 1.));
    }

    #[test]
    fn retained_f2_rational_projection_witness_bounds_are_exact() {
        use super::*;
        use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};
        // Frozen bounds were independently checked by exact rational projection
        // in bench/results/f2-20261011-geometry.json; keep all eight inputs.
        let cases: [(usize, [u64; 4]); 8] = [
            (32, [0x0, 0x0, 0x0, 0x0]),
            (
                37,
                [
                    0x3f8ef6c1c33f179a,
                    0x3f2938fee5b28a03,
                    0x3ec43f08f7784620,
                    0x3e603aa997cea259,
                ],
            ),
            (
                64,
                [
                    0x3fec20000000000b,
                    0x3fe8b82000000014,
                    0x3fe5b9d42000001b,
                    0x3fe3185370200020,
                ],
            ),
            (
                32,
                [
                    0x401c000000000007,
                    0x404480000000000b,
                    0x406de00000000015,
                    0x4095c40000000015,
                ],
            ),
            (
                41,
                [
                    0x3fa534c98596e2ae,
                    0x3f57704bdaddd1d9,
                    0x3f0975dd6873e244,
                    0x3ebb9b02fa80b04d,
                ],
            ),
            (32, [0x0, 0x0, 0x0, 0x0]),
            (
                32,
                [
                    0x3ff0000000000005,
                    0x3ff000000000000a,
                    0x3ff000000000000f,
                    0x3ff0000000000014,
                ],
            ),
            (
                37,
                [
                    0x3f8ef6c1c33f179a,
                    0x3f2938fee5b28a03,
                    0x3ec43f08f7784620,
                    0x3e603aa997cea259,
                ],
            ),
        ];
        for (case, (rows, expected)) in cases.into_iter().enumerate() {
            let triples: Vec<[u32; 3]> = (0..rows)
                .map(|r| {
                    let codes = match case {
                        0 => [r % 2, r / 2 % 2, r / 4 % 2],
                        1 => [r % 3, r / 3 % 3, r / 9 % 3],
                        2 => [
                            usize::from(r >= 32),
                            usize::from(r >= 32) ^ usize::from(r == 0 || r == 32),
                            r % 2,
                        ],
                        3 => [
                            2 * (r / 16) + r % 2,
                            2 * (r / 16) + r / 2 % 2,
                            2 * (r / 16) + r / 4 % 2,
                        ],
                        4 => [r % 3, r / 3 % 4, r / 12 % 2],
                        5 => [0, r % 2, r / 2 % 3],
                        6 => [r / 4 % 2, r / 4 % 2, r % 2],
                        7 => [2 - r % 3, 2 - r / 3 % 3, 2 - r / 9 % 3],
                        _ => unreachable!(),
                    };
                    codes.map(|c| c as u32)
                })
                .collect();
            let codes: [Vec<u32>; 3] =
                std::array::from_fn(|c| triples.iter().map(|r| r[c]).collect());
            let levels = codes
                .each_ref()
                .map(|c| *c.iter().max().unwrap() as usize + 1);
            let indexes: Vec<_> = codes
                .iter()
                .zip(levels)
                .map(|(c, n)| LevelIndex::from_codes(c, n).unwrap())
                .collect();
            let sources: Vec<_> = codes
                .iter()
                .zip(levels)
                .zip(&indexes)
                .map(|((c, n), i)| ValidatedBorrowedInterceptSource::new(c, n, i).unwrap())
                .collect();
            let design =
                BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect())
                    .unwrap();
            let (geometry, _) = Geometry::inspect(&design, None).unwrap();
            assert_eq!(
                geometry.unwrap().bounds.map(f64::to_bits),
                expected,
                "F2 case {case}"
            );
        }
    }
}
