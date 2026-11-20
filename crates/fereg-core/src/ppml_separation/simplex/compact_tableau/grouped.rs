//! Exact repeated projected constraints share a column in the transposed
//! program. Give every occurrence its unit baseline and put each group's
//! additional mass on its first original row. The caller rechecks the full
//! cone and raw design independently.
use super::{fallible_filled, solve, Cone, Proposal};
use crate::error::Result;
use std::hash::{DefaultHasher, Hash, Hasher};

const WORK: usize = 128 << 20;

pub(super) fn retry(
    rows: &[Vec<f64>],
    tolerance: f64,
    max_iterations: usize,
    pivots: &mut usize,
    available: usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Proposal>> {
    let Some(groups) = collect(rows, available, interrupt)? else {
        return Ok(None);
    };
    let Some(left) = available.checked_sub(2 * groups.capacity()) else {
        return Ok(None);
    };
    let cone = Cone {
        rows,
        groups: &groups,
        stored_once: false,
    };
    // The separate retry allows at most WORK tableau visits, even when the
    // direct solve exhausted its visit allowance. Both spend the same pivots.
    let result = solve(
        cone,
        tolerance,
        max_iterations,
        pivots,
        left,
        WORK,
        interrupt,
    )?;
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!("FEREG_SOLVER_TRACE|phase=compact_grouped_tableau|boundary={}|groups={}|dimension={}|pivots={pivots}|proposed={}",
            rows.len(), groups.len(), rows[0].len(), u8::from(result.is_some()));
    }
    expand(result, &groups, rows.len(), left)
}

pub(super) fn expand(
    result: Option<Proposal>,
    groups: &[(usize, usize)],
    boundary: usize,
    available: usize,
) -> Result<Option<Proposal>> {
    let Some(Proposal::PositiveWeights {
        weights,
        iterations,
    }) = result
    else {
        return Ok(result);
    };
    if boundary.saturating_add(weights.capacity()) > available {
        return Ok(None);
    }
    let mut expanded = fallible_filled(boundary, 1., "unable to reserve expanded dual weights")?;
    for (i, (&(rep, count), mass)) in groups.iter().zip(weights).enumerate() {
        crate::interrupt::row_checkpoint(i)?;
        let extra = mass - count as f64;
        if !extra.is_finite() || extra < 0. {
            return Ok(None);
        }
        expanded[rep] += extra;
    }
    Ok(Some(Proposal::PositiveWeights {
        weights: expanded,
        iterations,
    }))
}

#[derive(Clone, Copy, Default)]
struct Bucket {
    first: usize,
    count: usize,
    next: usize,
}

/// Linked occupied buckets retain first-occurrence order across hash table
/// sizes. Hashes only locate candidates: equality compares all coordinates.
fn collect(
    rows: &[Vec<f64>],
    available: usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Vec<(usize, usize)>>> {
    let slots = rows.len().min(available / 5).min(WORK / 5);
    if slots == 0 || rows.len() < 2 || rows.len() as u128 > 1u128 << 53 {
        return Ok(None);
    }
    interrupt()?;
    let dimension = rows[0].len();
    let mut buckets = fallible_filled(slots, Bucket::default(), "unable to reserve cone groups")?;
    let mut work = WORK - 5 * slots;
    let mut first = 0;
    let mut last = 0;
    let mut distinct = 0;
    'rows: for (i, row) in rows.iter().enumerate() {
        if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        let Some(left) = work.checked_sub(1 + dimension) else {
            return Ok(None);
        };
        work = left;
        if row.len() != dimension {
            return Ok(None);
        }
        let mut hash = DefaultHasher::new();
        for &x in row {
            if !x.is_finite() {
                return Ok(None);
            }
            let bits = if x == 0. { 0 } else { x.to_bits() };
            bits.hash(&mut hash);
        }
        let mut slot = (hash.finish() % slots as u64) as usize;
        for probe in 0..slots {
            crate::interrupt::row_checkpoint(probe)?;
            let Some(left) = work.checked_sub(1 + dimension) else {
                return Ok(None);
            };
            work = left;
            let bucket = buckets[slot];
            if bucket.count == 0 {
                buckets[slot] = Bucket {
                    first: i,
                    count: 1,
                    next: 0,
                };
                if distinct == 0 {
                    first = slot;
                } else {
                    buckets[last].next = slot;
                }
                last = slot;
                distinct += 1;
                continue 'rows;
            }
            if rows[bucket.first] == *row {
                buckets[slot].count += 1;
                continue 'rows;
            }
            slot = (slot + 1) % slots;
        }
        return Ok(None);
    }
    if distinct == rows.len() {
        return Ok(None);
    }
    // Three cells per bucket coexist with two cells per distinct row. The
    // tableau subsequently borrows rows from the caller without copying them.
    let mut groups = Vec::new();
    super::super::reserve(
        &mut groups,
        distinct,
        "unable to reserve repeated cone indices",
    )?;
    for i in 0..distinct {
        crate::interrupt::row_checkpoint(i)?;
        let b = buckets[first];
        groups.push((b.first, b.count));
        first = b.next;
    }
    Ok(Some(groups))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ppml_separation::simplex::compact_tableau::{propose, workspace_cells};

    #[test]
    fn group_multiplicities_balance_every_original_boundary_row() {
        let mut rows = vec![vec![2., 0.]; 40];
        rows.extend(vec![vec![-1., 0.]; 2]);
        rows.extend(vec![vec![0., 1.]; 3]);
        rows.extend(vec![vec![-0., -1.]; 12]);
        let limit = 2 * rows.len() + 32;
        assert!(workspace_cells(rows.len(), 2).unwrap() > limit);
        let Some(Proposal::PositiveWeights {
            weights,
            iterations,
        }) = propose(&rows, 1e-10, 2, limit, &mut || Ok(())).unwrap()
        else {
            panic!("expected two independently balanced coordinates");
        };
        assert_eq!(iterations, 2);
        assert_eq!(weights.len(), rows.len());
        for (i, &weight) in weights.iter().enumerate() {
            assert_eq!(
                weight,
                match i {
                    40 => 79.,
                    42 => 10.,
                    _ => 1.,
                }
            );
        }
        for j in 0..2 {
            assert_eq!(
                rows.iter()
                    .zip(&weights)
                    .map(|(r, w)| r[j] * w)
                    .sum::<f64>(),
                0.
            );
        }
        assert!(propose(&rows, 1e-10, 1, limit, &mut || Ok(()))
            .unwrap()
            .is_none());
    }

    #[test]
    fn grouped_ray_separates_all_copies_of_the_strict_constraint() {
        let mut rows = vec![vec![1., -1.]; 32];
        rows.extend(vec![vec![-1., 1.]; 20]);
        rows.extend(vec![vec![2., -1.]; 12]);
        let limit = 2 * rows.len() + 32;
        assert!(workspace_cells(rows.len(), 2).unwrap() > limit);
        let Some(Proposal::Direction { coordinates, .. }) =
            propose(&rows, 1e-10, 64, limit, &mut || Ok(())).unwrap()
        else {
            panic!("expected separating ray on every copy of the last row");
        };
        for (i, row) in rows.iter().enumerate() {
            let score = row[0] * coordinates[0] + row[1] * coordinates[1];
            if i < 52 {
                assert_eq!(score, 0.);
            } else {
                assert!(score < 0.);
            }
        }
    }

    #[test]
    fn signed_zeros_match_but_one_ulp_differences_stay_distinct() {
        let rows = [vec![1., -0.], vec![1. + f64::EPSILON, 0.], vec![1., 0.]];
        assert_eq!(
            collect(&rows, 64, &mut || Ok(())).unwrap(),
            Some(vec![(0, 2), (1, 1)])
        );
    }

    #[test]
    fn repeated_independent_balances_fit_below_the_full_tableau_bound() {
        let axes = 32;
        let repeats = 1024;
        let rows: Vec<_> = (0..axes * repeats)
            .map(|i| {
                let mut row = vec![0.; axes];
                row[i % axes] = if i / axes % 4 == 3 { -1. } else { 1. };
                row
            })
            .collect();
        assert!(workspace_cells(rows.len(), axes).unwrap() > 1_000_000);
        let Some(Proposal::PositiveWeights {
            weights,
            iterations,
        }) = propose(&rows, 1e-10, axes, 2 * rows.len() + 1, &mut || Ok(())).unwrap()
        else {
            panic!("a tableau with two distinct constraints per axis fits");
        };
        assert_eq!(iterations, axes);
        assert_eq!(weights.len(), rows.len());
        let mut moments = vec![0.; axes];
        for (i, &w) in weights.iter().enumerate() {
            assert!(w >= 1.);
            moments[i % axes] += rows[i][i % axes] * w;
        }
        assert!(moments.iter().all(|&v| v == 0.));
    }
}
