//! Construct an integer dual for a pure pair of categorical intercepts.
//! Zero outcomes require positive flow from left to right. Positive outcomes
//! allow signed flow, so their edges are traversable in both directions.
use super::*;

pub(super) fn certified(
    model: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    cells: usize,
    tolerance: f64,
) -> Result<bool> {
    if !pure_pair(model, topology) {
        return Ok(false);
    }
    let [left, right] = [topology.effect_to_source[0], topology.effect_to_source[1]];
    let indexes = [topology.sources[left], topology.sources[right]];
    let codes = [codes(model, left)?, codes(model, right)?];
    balanced(indexes, codes, model.outcome(), cells, tolerance)
}

fn balanced(
    indexes: [&LevelIndex; 2],
    codes: [&[u32]; 2],
    y: &[f64],
    cells: usize,
    tolerance: f64,
) -> Result<bool> {
    let split = indexes[0].n_levels();
    let vertices = split + indexes[1].n_levels();
    // One weight per physical observation and three reusable words per vertex.
    // A tree edge carries at most one unit per boundary row per traversal.
    if y.len().saturating_add(3usize.saturating_mul(vertices)) > cells
        || y.len() as u128 > (i64::MAX as u128 - 1) / 2
    {
        return Ok(false);
    }
    let mut weights = fallible_filled(y.len(), 0_i64, "unable to reserve graph dual weights")?;
    for (row, w) in weights.iter_mut().enumerate() {
        crate::interrupt::row_checkpoint(row)?;
        *w = i64::from(y[row] == 0.);
    }
    let mut parent_edge =
        fallible_filled(vertices, usize::MAX, "unable to reserve graph dual tree")?;
    let mut order = fallible_filled(vertices, 0usize, "unable to reserve graph traversal order")?;
    let mut load = fallible_filled(vertices, 0_i64, "unable to reserve graph dual flow")?;

    // For each zero edge (L,R), close a directed walk root -> L -> R -> root.
    // Add all such walks in two forest traversals instead of walking each path.
    // A component containing a one-way bridge can leave unmatched roots. The
    // independent final check rejects their imbalance.
    for reversed in [false, true] {
        parent_edge.fill(usize::MAX);
        load.fill(0);
        let mut len = 0;
        let mut visited = 0;
        for root in 0..vertices {
            crate::interrupt::row_checkpoint(root)?;
            if parent_edge[root] != usize::MAX {
                continue;
            }
            parent_edge[root] = y.len();
            let mut head = len;
            order[len] = root;
            len += 1;
            while head < len {
                let v = order[head];
                head += 1;
                let right = v >= split;
                let level = if right { v - split } else { v };
                let side = usize::from(right);
                for &row in indexes[side].checked_members(level)? {
                    crate::interrupt::row_checkpoint(visited)?;
                    visited += 1;
                    if y[row] == 0. {
                        if right != reversed {
                            continue;
                        }
                        load[v] += 1;
                    }
                    let other = codes[1 - side][row] as usize + if right { 0 } else { split };
                    if parent_edge[other] == usize::MAX {
                        parent_edge[other] = row;
                        order[len] = other;
                        len += 1;
                    }
                }
            }
        }
        for (i, &v) in order.iter().rev().enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            let row = parent_edge[v];
            if row == y.len() {
                continue;
            }
            let right = v >= split;
            let parent = if right {
                codes[0][row] as usize
            } else {
                split + codes[1][row] as usize
            };
            let flow = load[v];
            load[parent] += flow;
            weights[row] += if right != reversed { flow } else { -flow };
        }
    }
    // Replay the signed measure on each original FE level. With no other
    // slopes, these equalities also certify the structural/finite constant.
    let mut margin = i64::MAX;
    let mut norm2 = 0.0;
    for (row, (&outcome, &weight)) in y.iter().zip(&weights).enumerate() {
        crate::interrupt::row_checkpoint(row)?;
        norm2 += (weight as f64).powi(2);
        if outcome == 0. {
            margin = margin.min(weight);
        }
    }
    if margin as f64 <= tolerance * norm2.sqrt() {
        return Ok(false);
    }
    for index in indexes {
        let mut visited = 0;
        for level in 0..index.n_levels() {
            let mut moment = 0_i128;
            for &row in index.checked_members(level)? {
                crate::interrupt::row_checkpoint(visited)?;
                visited += 1;
                moment += i128::from(weights[row]);
            }
            if moment != 0 {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_cycle_closure_on_distinct_growing_components() {
        for levels in [3, 37, 1600] {
            let left: Vec<_> = (0..2 * levels).map(|r| (r % levels) as u32).collect();
            let right: Vec<_> = (0..2 * levels)
                .map(|r| ((r + usize::from(r >= levels)) % levels) as u32)
                .collect();
            let y: Vec<_> = (0..2 * levels).map(|r| f64::from(r < levels)).collect();
            for n in [2 * levels - 1, 2 * levels] {
                let g = LevelIndex::from_codes(&left[..n], levels).unwrap();
                let h = LevelIndex::from_codes(&right[..n], levels).unwrap();
                let check = |limit| {
                    balanced([&g, &h], [&left[..n], &right[..n]], &y[..n], limit, 1e-10).unwrap()
                };
                assert!(!check(n + 6 * levels - 1));
                assert_eq!(check(n + 6 * levels), n == 2 * levels);
                if n == 2 * levels {
                    let test_margin = |t| {
                        balanced(
                            [&g, &h],
                            [&left[..n], &right[..n]],
                            &y[..n],
                            n + 6 * levels,
                            t,
                        )
                        .unwrap()
                    };
                    let normalized = 1. / (n as f64).sqrt();
                    assert!(test_margin(normalized * (1. - 1e-8)));
                    assert!(!test_margin(normalized * (1. + 1e-8)));
                }
            }
        }
    }
}
