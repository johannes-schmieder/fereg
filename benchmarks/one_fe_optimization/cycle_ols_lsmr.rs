//! Captured-source experiment; not a production automatic-routing policy.
use crate::error::Result;
use crate::fe::{BorrowedInterceptFeDesign, BorrowedMapWeights};
use crate::memory::FeSolverMemoryBudget;

/// Recognize a single cycle in the smaller-axis Schur adjacency. The two
/// neighbors per vertex are exact, independent of labels and row ordering.
/// Reject more complicated graphs immediately; no adjacency expansion or
/// approximation is retained for them.
pub(super) fn probe(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    budget: Option<FeSolverMemoryBudget>,
) -> Result<(bool, u128)> {
    if design.effects().len() != 2 || design.nobs() < 16_384 {
        return Ok((false, 0));
    }
    let effects = design.effects();
    let (left, right) = if effects[0].n_levels() >= effects[1].n_levels() {
        (&effects[0], &effects[1])
    } else {
        (&effects[1], &effects[0])
    };
    let levels = right.n_levels();
    if levels < 256 || levels >= u32::MAX as usize
        || left.n_levels() / levels < 10
        || design.nobs() / left.n_levels() != 3
    {
        return Ok((false, 0));
    }
    let planned = levels as u128 * std::mem::size_of::<[u32; 2]>() as u128;
    if budget.is_some_and(|b| !b.fits(planned)) {
        return Ok((false, 0));
    }
    let mut neighbors = Vec::new();
    if neighbors.try_reserve_exact(levels).is_err() {
        return Ok((false, 0));
    }
    let actual = neighbors.capacity() as u128 * std::mem::size_of::<[u32; 2]>() as u128;
    if budget.is_some_and(|b| !b.fits(actual)) {
        return Ok((false, actual));
    }
    neighbors.resize(levels, [u32::MAX; 2]);
    for level in 0..left.n_levels() {
        let rows = left.level_index().members(level);
        // Restrict the first experiment to the measured three-row partition.
        // A wider range needs separate timing and scientific evidence.
        if rows.len() != 3 {
            return Ok((false, actual));
        }
        let a = right.codes()[rows[0]];
        let mut b = a;
        for &row in &rows[1..] {
            let code = right.codes()[row];
            if code != a {
                if b != a && b != code {
                    return Ok((false, actual));
                }
                b = code;
            }
        }
        if a == b {
            continue;
        }
        for (vertex, neighbor) in [(a, b), (b, a)] {
            let adjacent = &mut neighbors[vertex as usize];
            if adjacent.contains(&neighbor) {
                continue;
            }
            if adjacent[0] == u32::MAX {
                adjacent[0] = neighbor;
            } else if adjacent[1] == u32::MAX {
                adjacent[1] = neighbor;
            } else {
                return Ok((false, actual));
            }
        }
    }
    if neighbors.iter().any(|adjacent| adjacent[1] == u32::MAX) {
        return Ok((false, actual));
    }
    // A finite undirected simple graph with degree two is a union of cycles.
    // Walking one cycle proves connectedness without another visited bank.
    let mut previous = 0;
    let mut current = neighbors[0][0] as usize;
    let mut visited = 1;
    while current != 0 && visited < levels {
        let adjacent = neighbors[current];
        let next = if adjacent[0] as usize == previous { adjacent[1] } else { adjacent[0] };
        previous = current;
        current = next as usize;
        visited += 1;
    }
    if current != 0 || visited != levels {
        return Ok((false, actual));
    }
    if let BorrowedMapWeights::Provided(weights) = weights {
        if weights.len() != design.nobs() {
            return Ok((false, actual));
        }
        let mut low = f64::INFINITY;
        let mut high = 0.0_f64;
        for &weight in weights {
            if !weight.is_finite() || weight <= 0.0 {
                return Ok((false, actual));
            }
            low = low.min(weight);
            high = high.max(weight);
        }
        if low / high < 0.125 {
            return Ok((false, actual));
        }
    }
    Ok((true, actual))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

    #[test]
    fn cycle_guard_is_exact_under_relabeling_and_rejects_other_graphs_or_budgets() {
        let levels = 256;
        let groups = levels * 24;
        let n = groups * 3;
        for variant in 0..5 {
            for permuted in [false, true] {
                let mut left = Vec::new();
                let mut right = Vec::new();
                for row in 0..n {
                    let group = row / 3;
                    let base = group % levels;
                    let next = if variant == 1 { (base / 128) * 128 + (base + 1) % 128 }
                        else { (base + 1) % levels };
                    let firm = match row % 3 {
                        1 => next,
                        2 if variant == 2 => (base + 17) % levels,
                        _ => base,
                    };
                    left.push(if permuted { ((group * 17 + 3) % groups) as u32 } else { group as u32 });
                    right.push(if permuted { ((firm * 37 + 11) % levels) as u32 } else { firm as u32 });
                }
                if variant == 3 {
                    // Break the last edge of the ring, leaving a path.
                    for row in 0..n {
                        if row / 3 % levels == levels - 1 && row % 3 == 1 {
                            let firm = levels - 1;
                            right[row] = if permuted { ((firm * 37 + 11) % levels) as u32 } else { firm as u32 };
                        }
                    }
                }
                if variant == 4 {
                    // Keep a connected cycle but leave the measured group-size
                    // envelope through a two/four-row pair of left groups.
                    left[0] = left[3];
                }
                if permuted {
                    left.reverse();
                    right.reverse();
                }
                let li = LevelIndex::from_codes(&left, groups).unwrap();
                let ri = LevelIndex::from_codes(&right, levels).unwrap();
                let l = ValidatedBorrowedInterceptSource::new(&left, groups, &li).unwrap();
                let r = ValidatedBorrowedInterceptSource::new(&right, levels, &ri).unwrap();
                for reverse in [false, true] {
                    let design = BorrowedInterceptFeDesign::new(n, if reverse {
                        vec![r.effect(), l.effect()]
                    } else { vec![l.effect(), r.effect()] }).unwrap();
                    for scale in [None, Some(1e-100), Some(1.0), Some(1e100)] {
                        let raw = scale.map(|scale| (0..n).map(|r| scale * (1 + r % 5) as f64).collect::<Vec<_>>());
                        let weights = raw.as_deref().map_or(BorrowedMapWeights::Unit, BorrowedMapWeights::Provided);
                        let (selected, peak) = probe(&design, weights, None).unwrap();
                        assert_eq!(selected, variant == 0);
                        assert!(peak >= levels as u128 * 8);
                    }
                    let usable = 512 + levels as u128 * 8;
                    for short in [false, true] {
                        let (selected, peak) = probe(&design, BorrowedMapWeights::Unit,
                            Some(FeSolverMemoryBudget { execution_bytes: 512,
                                usable_bytes: usable - u128::from(short) })).unwrap();
                        assert_eq!(selected, variant == 0 && !short);
                        if short { assert_eq!(peak, 0); }
                    }
                    let raw: Vec<_> = (0..n).map(|r| if r == 0 { 1e-8 } else { 1.0 }).collect();
                    assert!(!probe(&design, BorrowedMapWeights::Provided(&raw), None).unwrap().0);
                }
            }
        }
    }
}
