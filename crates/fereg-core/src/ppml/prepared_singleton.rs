//! Singleton fixed point on the immutable prepared generation's existing CSR.

use super::{allocation, bounded_push, fallible_filled, ErrorCode, FeregError, LevelIndex, Result};

/// Frequency weights have already passed canonical positive-integer validation.
/// Probability weights count physical rows, as in the original pass algorithm.
/// Returned indices are sorted on the compact generation axis.
pub(super) fn prune(
    sources: &[(&[u32], &LevelIndex)],
    mut active: Vec<bool>,
    frequency: Option<&[f64]>,
) -> Result<Vec<usize>> {
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    let contribution = |row: usize| frequency.map_or(1_u128, |weights| weights[row] as u128);
    let mut offsets = Vec::new();
    let offset_capacity = sources
        .len()
        .checked_add(1)
        .ok_or_else(|| allocation("prepared singleton offset capacity overflowed"))?;
    offsets
        .try_reserve_exact(offset_capacity)
        .map_err(|_| allocation("unable to reserve prepared singleton offsets"))?;
    offsets.push(0_usize);
    for (_, members) in sources {
        let next = offsets
            .last()
            .unwrap()
            .checked_add(members.n_levels())
            .ok_or_else(|| allocation("prepared singleton level capacity overflowed"))?;
        bounded_push(
            &mut offsets,
            offset_capacity,
            next,
            "prepared singleton offset capacity exhausted",
        )?;
    }
    let mut mass = fallible_filled(
        *offsets.last().unwrap(),
        0_u128,
        "unable to reserve prepared singleton multiplicities",
    )?;
    for (source, (_, members)) in sources.iter().enumerate() {
        for level in 0..members.n_levels() {
            let rows = members.checked_members(level)?;
            #[cfg(test)]
            MEMBER_VISITS.with(|visits| visits.set(visits.get() + rows.len()));
            mass[offsets[source] + level] = if let Some(weights) = frequency {
                let mut count = 0_u128;
                for &row in rows {
                    if active[row] {
                        count = count.checked_add(weights[row] as u128).ok_or_else(|| {
                            FeregError::new(
                                ErrorCode::InvalidData,
                                "prepared singleton multiplicity overflowed",
                            )
                        })?;
                    }
                }
                count
            } else {
                // Physical row counts cannot overflow usize: each level's
                // validated membership is a subset of the observation axis.
                rows.iter().filter(|&&row| active[row]).count() as u128
            };
        }
    }
    // Balanced samples never allocate the deletion queue or selection mask.
    if !mass.contains(&1) {
        return Ok(Vec::new());
    }
    let n = active.len();
    let mut dropped = Vec::new();
    dropped
        .try_reserve_exact(n)
        .map_err(|_| allocation("unable to reserve prepared singleton drop rows"))?;
    let mut selected = fallible_filled(
        n,
        false,
        "unable to reserve prepared singleton selection mask",
    )?;
    for (source, (_, members)) in sources.iter().enumerate() {
        for level in 0..members.n_levels() {
            if mass[offsets[source] + level] == 1 {
                enqueue(members, level, &mut active, &mut selected, &mut dropped)?;
            }
        }
    }
    let mut cursor = 0;
    while cursor < dropped.len() {
        let row = dropped[cursor];
        cursor += 1;
        let removed = contribution(row);
        for (source, (codes, members)) in sources.iter().enumerate() {
            let level = codes[row] as usize;
            let count = &mut mass[offsets[source] + level];
            *count = count.checked_sub(removed).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "prepared singleton multiplicity underflowed",
                )
            })?;
            if *count == 1 {
                enqueue(members, level, &mut active, &mut selected, &mut dropped)?;
            }
        }
    }
    // Reuse the queue to publish sorted rows in O(N), without an N log N sort
    // or confusing previously inactive rows with this transaction's deletions.
    dropped.clear();
    for (row, &was_selected) in selected.iter().enumerate() {
        if was_selected {
            bounded_push(
                &mut dropped,
                n,
                row,
                "prepared singleton output capacity exhausted",
            )?;
        }
    }
    Ok(dropped)
}

fn enqueue(
    members: &LevelIndex,
    level: usize,
    active: &mut [bool],
    selected: &mut [bool],
    queue: &mut Vec<usize>,
) -> Result<()> {
    // A level reaches mass one at most once. Its CSR slice is therefore scanned
    // at most once after counting, even on a chain requiring O(N) old passes.
    // A still-counted member may already be queued through another effect.
    if let Some(&row) = members.checked_members(level)?.iter().find(|&&row| {
        #[cfg(test)]
        MEMBER_VISITS.with(|visits| visits.set(visits.get() + 1));
        active[row]
    }) {
        bounded_push(
            queue,
            active.len(),
            row,
            "prepared singleton queue capacity exhausted",
        )?;
        active[row] = false;
        selected[row] = true;
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    static MEMBER_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent simultaneous-deletion reference retained from the prepared
    // pass implementation. No counters, queue or row-to-level lookup is shared.
    fn reference(
        sources: &[(&[u32], &LevelIndex)],
        mut active: Vec<bool>,
        frequency: Option<&[f64]>,
    ) -> Vec<usize> {
        let mut dropped = Vec::new();
        loop {
            let mut selected = vec![false; active.len()];
            for (_, members) in sources {
                for level in 0..members.n_levels() {
                    let mut mass = 0_u128;
                    let mut sole = None;
                    for &row in members.members(level) {
                        if active[row] {
                            mass += frequency.map_or(1, |w| w[row] as u128);
                            sole = Some(row);
                        }
                    }
                    if mass == 1 {
                        selected[sole.unwrap()] = true;
                    }
                }
            }
            let mut any = false;
            for row in 0..active.len() {
                if selected[row] && active[row] {
                    active[row] = false;
                    dropped.push(row);
                    any = true;
                }
            }
            if !any {
                break;
            }
        }
        dropped.sort_unstable();
        dropped
    }

    fn compare(codes: Vec<Vec<u32>>, active: Vec<bool>, weights: Vec<f64>) {
        let indexes: Vec<_> = codes
            .iter()
            .map(|c| {
                LevelIndex::from_codes(c, c.iter().max().map_or(0, |v| *v as usize + 2)).unwrap()
            })
            .collect();
        let sources: Vec<_> = codes
            .iter()
            .zip(&indexes)
            .map(|(c, m)| (c.as_slice(), m))
            .collect();
        for frequency in [None, Some(weights.as_slice())] {
            let expected = reference(&sources, active.clone(), frequency);
            MEMBER_VISITS.with(|v| v.set(0));
            let actual = prune(&sources, active.clone(), frequency).unwrap();
            assert_eq!(actual, expected);
            assert!(actual.capacity() <= active.len());
            MEMBER_VISITS.with(|v| assert!(v.get() <= 2 * active.len() * sources.len()));
        }
    }

    #[test]
    fn exact_masks_match_pass_reference_across_multiway_weights_and_initial_exclusions() {
        let mut seed = 937_u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            seed >> 32
        };
        for case in 0..320 {
            let n = 1 + (next() % 91) as usize;
            let d = 1 + (next() % 5) as usize;
            let codes = (0..d)
                .map(|_| {
                    let levels = 1 + next() % n as u64;
                    (0..n).map(|_| (next() % levels) as u32).collect()
                })
                .collect();
            let active = (0..n).map(|_| next() % 7 != 0).collect();
            let weights = (0..n)
                .map(|row| {
                    if case % 13 == 0 && row % 3 == 0 {
                        4_503_599_627_370_496.0
                    } else {
                        (1 + next() % 3) as f64
                    }
                })
                .collect();
            compare(codes, active, weights);
        }
    }

    fn chain(n: usize) -> Vec<Vec<u32>> {
        // Alternating path plus a stable four-edge cycle. Every path edge drops.
        let mut a: Vec<_> = (0..n).map(|row| (row / 2) as u32).collect();
        let mut b: Vec<_> = (0..n).map(|row| ((row + 1) / 2) as u32).collect();
        let base = n as u32;
        a.extend([base, base, base + 1, base + 1]);
        b.extend([base, base + 1, base, base + 1]);
        vec![a, b]
    }

    #[test]
    fn cascading_masks_match_with_frequency_protected_rows_and_inactive_endpoints() {
        let codes = chain(512);
        let mut active = vec![true; 516];
        active[0] = false;
        active[217] = false;
        let mut weights = vec![1.0; 516];
        weights[101] = 2.0;
        weights[298] = 3.0;
        compare(codes, active, weights);
    }

    #[test]
    fn cascade_scale_doubling_bounds_membership_work_and_keeps_stable_cycle() {
        let mut last = 0;
        for n in [4096, 8192, 16384] {
            let codes = chain(n);
            let indexes: Vec<_> = codes
                .iter()
                .map(|c| LevelIndex::from_codes(c, n + 2).unwrap())
                .collect();
            let sources: Vec<_> = codes
                .iter()
                .zip(&indexes)
                .map(|(c, m)| (c.as_slice(), m))
                .collect();
            MEMBER_VISITS.with(|v| v.set(0));
            assert_eq!(
                prune(&sources, vec![true; n + 4], None).unwrap(),
                (0..n).collect::<Vec<_>>()
            );
            let visits = MEMBER_VISITS.with(|v| v.get());
            assert!(visits <= 4 * (n + 4));
            if last > 0 {
                assert!(visits <= 2 * last);
            }
            last = visits;
        }
    }

    #[test]
    fn no_singleton_or_no_effect_has_no_drop_allocation() {
        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).unwrap();
        let dropped = prune(&[(&codes, &index)], vec![true; 4], None).unwrap();
        assert_eq!(dropped.capacity(), 0);
        assert_eq!(prune(&[], vec![false, true], None).unwrap().capacity(), 0);
        let weights = [2.0; 4];
        assert!(prune(
            &[(&codes, &index)],
            vec![true, false, true, false],
            Some(&weights)
        )
        .unwrap()
        .is_empty());
    }
}
