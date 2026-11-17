//! Exact recession-direction proposal from a two-intercept submodel.
//!
//! Positive rows identify the two endpoint potentials. On the resulting
//! quotient, a zero row imposes potential(left) <= potential(right). Strongly
//! connected components must share a potential; their topological order gives
//! a nonpositive direction on every zero row and a strict one across components.
//! This supplies an IR starting proposal, never a full-model no-separation proof.

use super::{allocation, fallible_copy, fallible_filled, invalid_argument};
use crate::error::Result;
use crate::fe::BorrowedInterceptFeDesign;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ExactInterceptSpan {
    effects: [usize; 2],
    split: usize,
    potentials: Vec<usize>,
}

pub(super) struct GraphDirection {
    pub(super) direction: Vec<f64>,
    pub(super) span: ExactInterceptSpan,
}

struct PairDirection {
    direction: Vec<f64>,
    split: usize,
    potentials: Vec<usize>,
}

impl ExactInterceptSpan {
    /// Replay the coefficient product through group membership, independently
    /// of the detector's row-code/quotient traversal. Integers below 2^53 and
    /// their differences are exactly representable in the returned direction.
    pub(super) fn verify(
        &self,
        design: &BorrowedInterceptFeDesign<'_>,
        direction: &[f64],
    ) -> Result<()> {
        if self.effects[0] >= self.effects[1]
            || self.effects[1] >= design.effects().len()
            || direction.len() != design.nobs()
            || self.split != design.effects()[self.effects[0]].n_levels()
            || self.potentials.len() != self.split + design.effects()[self.effects[1]].n_levels()
            || self
                .potentials
                .iter()
                .any(|&value| value as u128 > (1_u128 << 53))
        {
            return Err(invalid_argument(
                "exact separation coefficient proof is not aligned",
            ));
        }
        let mut reconstructed = fallible_filled(
            design.nobs(),
            0_i64,
            "unable to reserve exact separation coefficient replay",
        )?;
        for (effect, offset, sign) in [
            (design.effects()[self.effects[0]], 0, 1_i64),
            (design.effects()[self.effects[1]], self.split, -1_i64),
        ] {
            for level in 0..effect.n_levels() {
                let value = sign * self.potentials[offset + level] as i64;
                for &row in effect.level_index().checked_members(level)? {
                    reconstructed[row] += value;
                }
            }
        }
        if reconstructed
            .iter()
            .zip(direction)
            .any(|(&value, &actual)| value as f64 != actual)
        {
            return Err(invalid_argument(
                "exact separation coefficient replay disagrees with direction",
            ));
        }
        Ok(())
    }
}

pub(super) fn workspace_bytes(design: &BorrowedInterceptFeDesign<'_>) -> Result<Option<u128>> {
    if design.effects().len() < 2 {
        return Ok(None);
    }
    let mut largest = [0_u128; 2];
    for effect in design.effects() {
        let levels = effect.n_levels() as u128;
        if levels > largest[0] {
            largest[1] = largest[0];
            largest[0] = levels;
        } else {
            largest[1] = largest[1].max(levels);
        }
    }
    // A finite binary column contributes a virtual two-level partition.
    let levels = largest[0]
        .checked_add(largest[1].max(2))
        .ok_or_else(|| allocation("separation graph levels overflow"))?;
    // Integer potentials and their differences must be exactly representable.
    if levels > (1_u128 << 53) {
        return Ok(None);
    }
    // DSU, dual CSR, traversal, finishing order, component labels, returned
    // direction and proposal overlap, including one reusable u32 binary-code
    // bank. Peak pair storage is at most 57*levels + 28*N + 16 bytes on
    // 64-bit targets (two edge arrays, one direction, binary codes); this
    // bound also covers earlier cursor phases and returned proposal overlap.
    // No recursion, binary membership index, or N-by-column materialization.
    let bytes = levels
        .checked_mul(96)
        .and_then(|n| {
            (design.nobs() as u128)
                .checked_mul(32)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| allocation("separation graph workspace overflows"))?;
    Ok(Some(bytes))
}

fn root(parent: &mut [usize], mut v: usize) -> usize {
    while parent[v] != v {
        parent[v] = parent[parent[v]];
        v = parent[v];
    }
    v
}

pub(super) fn direction(
    design: &BorrowedInterceptFeDesign<'_>,
    outcome: &[f64],
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<GraphDirection>> {
    if workspace_bytes(design)?.is_none() {
        return Ok(None);
    }
    // Preserve the established first-pair proposal, then search a star about
    // the finest categorical partition. At most J pairs for J effects bounds
    // work by O(J*(N+levels)), rather than an exhaustive quadratic pair scan.
    // Missing a submodel direction cannot certify absence: full-model IR and
    // its independent unit-metric dual check still follow this accelerator.
    if let Some(candidate) = categorical_pair_direction(design, [0, 1], outcome, interrupt)? {
        return Ok(Some(candidate));
    }
    let mut anchor = 0;
    for effect in 1..design.effects().len() {
        if design.effects()[effect].n_levels() > design.effects()[anchor].n_levels() {
            anchor = effect;
        }
    }
    for other in 0..design.effects().len() {
        if other == anchor {
            continue;
        }
        let pair = [anchor.min(other), anchor.max(other)];
        if pair == [0, 1] {
            continue;
        }
        if let Some(candidate) = categorical_pair_direction(design, pair, outcome, interrupt)? {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

/// Every binary level indicator is in the span of that original finite
/// column and the categorical intercept's constant. This is only a primal
/// proposal: the caller must independently certify its full-model span.
pub(super) fn binary_direction(
    design: &BorrowedInterceptFeDesign<'_>,
    outcome: &[f64],
    columns: usize,
    value: &mut impl FnMut(usize, usize) -> Result<f64>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Vec<f64>>> {
    if workspace_bytes(design)?.is_none() || columns == 0 {
        return Ok(None);
    }
    interrupt()?;
    let anchor = design
        .effects()
        .iter()
        .enumerate()
        .fold(0, |best, (index, effect)| {
            if effect.n_levels() > design.effects()[best].n_levels() {
                index
            } else {
                best
            }
        });
    let effect = design.effects()[anchor];
    let mut binary = fallible_filled(design.nobs(), 0_u32, "unable to reserve binary graph codes")?;
    // At most one scan and one graph per generated column, using the same
    // N-sized code bank. No failure to propose can certify NoSeparation.
    for column in 0..columns {
        interrupt()?;
        let mut levels = 0_u8;
        for (row, code) in binary.iter_mut().enumerate() {
            if row % 65_536 == 0 {
                interrupt()?;
            }
            let raw = value(column, row)?;
            if raw == 0. {
                *code = 0;
                levels |= 1;
            } else if raw == 1. {
                *code = 1;
                levels |= 2;
            } else {
                levels = 0;
                break;
            }
        }
        if levels != 3 {
            continue;
        }
        if let Some(candidate) = pair_direction(
            [effect.codes(), &binary],
            [effect.n_levels(), 2],
            outcome,
            interrupt,
        )? {
            // Potentials and binary scratch die before independent numerical
            // span certification. Do not manufacture an exact FE-only proof.
            return Ok(Some(candidate.direction));
        }
    }
    Ok(None)
}

fn categorical_pair_direction(
    design: &BorrowedInterceptFeDesign<'_>,
    effects: [usize; 2],
    outcome: &[f64],
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<GraphDirection>> {
    let left = design.effects()[effects[0]];
    let right = design.effects()[effects[1]];
    Ok(pair_direction(
        [left.codes(), right.codes()],
        [left.n_levels(), right.n_levels()],
        outcome,
        interrupt,
    )?
    .map(|candidate| GraphDirection {
        direction: candidate.direction,
        span: ExactInterceptSpan {
            effects,
            split: candidate.split,
            potentials: candidate.potentials,
        },
    }))
}

fn pair_direction(
    codes: [&[u32]; 2],
    level_counts: [usize; 2],
    outcome: &[f64],
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<PairDirection>> {
    let invalid_outcome = || {
        crate::parallel::any_index(outcome.len(), |row| {
            !outcome[row].is_finite() || outcome[row] < 0.
        })
    };
    if codes.iter().any(|values| values.len() != outcome.len()) || invalid_outcome() {
        return Err(invalid_argument(
            "separation graph outcomes are not aligned and nonnegative",
        ));
    }
    interrupt()?;
    let split = level_counts[0];
    let levels = split
        .checked_add(level_counts[1])
        .ok_or_else(|| allocation("separation graph levels overflow"))?;
    if levels as u128 > (1_u128 << 53) {
        return Ok(None);
    }
    for side in 0..2 {
        for (row, &code) in codes[side].iter().enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            if code as usize >= level_counts[side] {
                return Err(invalid_argument(
                    "separation graph code exceeds its partition",
                ));
            }
        }
    }
    let offsets = levels
        .checked_add(1)
        .ok_or_else(|| allocation("separation graph offsets overflow"))?;
    let mut parent = fallible_filled(
        levels,
        0_usize,
        "unable to reserve separation graph parents",
    )?;
    for (v, p) in parent.iter_mut().enumerate() {
        if v % 65_536 == 0 {
            interrupt()?;
        }
        *p = v;
    }
    let mut rank = fallible_filled(levels, 0_u8, "unable to reserve separation graph ranks")?;
    // A positive row repeating the previous positive row's code pair is a
    // no-op union; skipping it leaves the linking sequence and every root
    // unchanged.
    let mut previous = (u32::MAX, u32::MAX);
    for (row, &y) in outcome.iter().enumerate() {
        if row % 65_536 == 0 {
            interrupt()?;
        }
        if y > 0. {
            let pair = (codes[0][row], codes[1][row]);
            if pair == previous {
                continue;
            }
            previous = pair;
            let mut a = root(&mut parent, codes[0][row] as usize);
            let mut b = root(&mut parent, split + codes[1][row] as usize);
            if a != b {
                if rank[a] < rank[b] {
                    std::mem::swap(&mut a, &mut b);
                }
                parent[b] = a;
                if rank[a] == rank[b] {
                    rank[a] += 1;
                }
            }
        }
    }
    for v in 0..levels {
        if v % 65_536 == 0 {
            interrupt()?;
        }
        parent[v] = root(&mut parent, v);
    }
    drop(rank);
    let endpoints = |row: usize| {
        (
            parent[codes[0][row] as usize],
            parent[split + codes[1][row] as usize],
        )
    };
    let mut forward = fallible_filled(
        offsets,
        0_usize,
        "unable to reserve separation graph forward offsets",
    )?;
    let mut reverse = fallible_filled(
        offsets,
        0_usize,
        "unable to reserve separation graph reverse offsets",
    )?;
    for (row, &y) in outcome.iter().enumerate() {
        if row % 65_536 == 0 {
            interrupt()?;
        }
        if y == 0. {
            let (a, b) = endpoints(row);
            if a != b {
                forward[a + 1] += 1;
                reverse[b + 1] += 1;
            }
        }
    }
    for v in 0..levels {
        if v % 65_536 == 0 {
            interrupt()?;
        }
        forward[v + 1] += forward[v];
        reverse[v + 1] += reverse[v];
    }
    if forward[levels] == 0 {
        return Ok(None);
    }
    let mut targets = fallible_filled(
        forward[levels],
        0_usize,
        "unable to reserve separation graph edges",
    )?;
    let mut origins = fallible_filled(
        reverse[levels],
        0_usize,
        "unable to reserve separation graph reverse edges",
    )?;
    let mut next = fallible_copy(
        &forward[..levels],
        "unable to reserve separation graph cursors",
    )?;
    let mut reverse_next = fallible_copy(
        &reverse[..levels],
        "unable to reserve separation graph reverse cursors",
    )?;
    for (row, &y) in outcome.iter().enumerate() {
        if row % 65_536 == 0 {
            interrupt()?;
        }
        if y == 0. {
            let (a, b) = endpoints(row);
            if a != b {
                targets[next[a]] = b;
                next[a] += 1;
                origins[reverse_next[b]] = a;
                reverse_next[b] += 1;
            }
        }
    }
    drop(reverse_next);
    next.copy_from_slice(&forward[..levels]);
    let mut seen = fallible_filled(levels, false, "unable to reserve separation graph visits")?;
    let mut stack = Vec::new();
    stack
        .try_reserve_exact(levels)
        .map_err(|_| allocation("unable to reserve separation graph traversal"))?;
    let mut order = Vec::new();
    order
        .try_reserve_exact(levels)
        .map_err(|_| allocation("unable to reserve separation graph finishing order"))?;
    for v in 0..levels {
        if v % 65_536 == 0 {
            interrupt()?;
        }
        if parent[v] != v || seen[v] {
            continue;
        }
        seen[v] = true;
        stack.push(v);
        while let Some(&a) = stack.last() {
            if next[a] == forward[a + 1] {
                order.push(a);
                stack.pop();
            } else {
                if next[a] % 65_536 == 0 {
                    interrupt()?;
                }
                let b = targets[next[a]];
                next[a] += 1;
                if !seen[b] {
                    seen[b] = true;
                    stack.push(b);
                }
            }
        }
    }
    let mut component = fallible_filled(
        levels,
        usize::MAX,
        "unable to reserve separation graph components",
    )?;
    let mut count = 0;
    for (index, &v) in order.iter().rev().enumerate() {
        if index % 65_536 == 0 {
            interrupt()?;
        }
        if component[v] != usize::MAX {
            continue;
        }
        component[v] = count;
        stack.push(v);
        while let Some(a) = stack.pop() {
            if a % 65_536 == 0 {
                interrupt()?;
            }
            for (edge, &b) in origins[reverse[a]..reverse[a + 1]].iter().enumerate() {
                if edge % 65_536 == 0 {
                    interrupt()?;
                }
                if component[b] == usize::MAX {
                    component[b] = count;
                    stack.push(b);
                }
            }
        }
        count += 1;
    }
    let mut direction = fallible_filled(
        outcome.len(),
        0.,
        "unable to reserve separation graph direction",
    )?;
    let mut strict = false;
    // Reconstruct from original endpoint codes, independently of CSR edges.
    // Extra model columns/FE effects have coefficient zero in this proposal.
    for (row, z) in direction.iter_mut().enumerate() {
        if row % 65_536 == 0 {
            interrupt()?;
        }
        let (a, b) = endpoints(row);
        *z = component[a] as f64 - component[b] as f64;
        if (outcome[row] > 0. && *z != 0.) || (outcome[row] == 0. && *z > 0.) {
            return Err(invalid_argument(
                "separation graph direction failed exact endpoint reconstruction",
            ));
        }
        strict |= *z < 0.;
    }
    if !strict {
        return Ok(None);
    }
    // Reuse the no-longer-needed DSU bank for the explicit level coefficients.
    // Graph traversal storage is dropped before the independent replay begins.
    for (level, potential) in parent.iter_mut().enumerate() {
        if level % 65_536 == 0 {
            interrupt()?;
        }
        *potential = component[*potential];
    }
    Ok(Some(PairDirection {
        direction,
        split,
        potentials: parent,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{BorrowedInterceptEffect, LevelIndex};

    fn run(g: &[u32], h: &[u32], y: &[f64]) -> Option<Vec<f64>> {
        let ng = *g.iter().max().unwrap() as usize + 1;
        let nh = *h.iter().max().unwrap() as usize + 1;
        let gi = LevelIndex::from_codes(g, ng).unwrap();
        let hi = LevelIndex::from_codes(h, nh).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            g.len(),
            vec![
                BorrowedInterceptEffect::new(g, ng, &gi).unwrap(),
                BorrowedInterceptEffect::new(h, nh, &hi).unwrap(),
            ],
        )
        .unwrap();
        direction(&design, y, &mut || Ok(()))
            .unwrap()
            .map(|candidate| {
                candidate
                    .span
                    .verify(&design, &candidate.direction)
                    .unwrap();
                candidate.direction
            })
    }

    fn binary(g: &[u32], h: &[u32], y: &[f64], x: &[f64]) -> Option<Vec<f64>> {
        let ng = *g.iter().max().unwrap() as usize + 1;
        let nh = *h.iter().max().unwrap() as usize + 1;
        let gi = LevelIndex::from_codes(g, ng).unwrap();
        let hi = LevelIndex::from_codes(h, nh).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            g.len(),
            vec![
                BorrowedInterceptEffect::new(g, ng, &gi).unwrap(),
                BorrowedInterceptEffect::new(h, nh, &hi).unwrap(),
            ],
        )
        .unwrap();
        binary_direction(&design, y, 1, &mut |_, row| Ok(x[row]), &mut || Ok(())).unwrap()
    }

    #[test]
    fn binary_control_proposes_exact_contrast_after_categorical_graph_misses() {
        let g = [0, 0, 1, 1, 2, 2, 2, 0];
        let h = [0, 1, 0, 1, 0, 0, 1, 0];
        let y = [1., 1., 1., 1., 1., 0., 0., 0.];
        let x = [0., 0., 1., 1., 1., 0., 0., 0.];
        assert!(run(&g, &h, &y).is_none());
        let z = binary(&g, &h, &y, &x).unwrap();
        // Independent coefficient witness: x - 1[g=1] - 1[g=2].
        let expected: Vec<_> = (0..8).map(|row| x[row] - f64::from(g[row] != 0)).collect();
        assert_eq!(z, expected);
        assert!(binary(&[0, 1, 0, 1], &[0; 4], &[1., 1., 0., 0.], &[0., 1., 1., 0.]).is_none());
        for nonbinary in [
            [0.; 8],
            [1.; 8],
            [2.; 8],
            [0., 0., 1. + 2_f64.powi(-50), 1., 1., 0., 0., 0.],
        ] {
            assert!(binary(&g, &h, &y, &nonbinary).is_none());
        }
    }

    #[test]
    fn binary_control_polls_break_before_reading_sources() {
        let g = [0, 1];
        let index = LevelIndex::from_codes(&g, 2).unwrap();
        let effect = BorrowedInterceptEffect::new(&g, 2, &index).unwrap();
        let design = BorrowedInterceptFeDesign::new(2, vec![effect, effect]).unwrap();
        let error = binary_direction(
            &design,
            &[1., 0.],
            1,
            &mut |_, _| panic!("interrupted proposal must not read sources"),
            &mut || {
                Err(crate::error::FeregError::new(
                    crate::error::ErrorCode::Interrupted,
                    "test Break",
                ))
            },
        )
        .unwrap_err();
        assert_eq!(error.code, crate::error::ErrorCode::Interrupted);
    }

    #[test]
    fn positive_pairs_with_one_way_zero_bridge_have_an_exact_direction() {
        let z = run(
            &[0, 0, 1, 1, 0, 0],
            &[0, 0, 1, 1, 1, 1],
            &[1., 1., 1., 1., 0., 0.],
        )
        .unwrap();
        assert_eq!(&z[..4], &[0.; 4]);
        assert!(z[4] < 0. && z[4] == z[5]);
    }

    #[test]
    fn a_reverse_zero_bridge_closes_the_cycle_and_forces_equality() {
        assert!(run(&[0, 1, 0, 1], &[0, 1, 1, 0], &[1., 1., 0., 0.]).is_none());
        assert!(run(&[0, 0, 1, 1], &[0, 0, 1, 1], &[1., 0., 1., 0.]).is_none());
    }

    #[test]
    fn coefficient_replay_rejects_forged_direction_and_stale_level_coordinates() {
        let g = [0, 0, 1, 1, 0, 0];
        let h = [0, 0, 1, 1, 1, 1];
        let gi = LevelIndex::from_codes(&g, 2).unwrap();
        let hi = LevelIndex::from_codes(&h, 2).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            g.len(),
            vec![
                BorrowedInterceptEffect::new(&g, 2, &gi).unwrap(),
                BorrowedInterceptEffect::new(&h, 2, &hi).unwrap(),
            ],
        )
        .unwrap();
        let candidate = direction(&design, &[1., 1., 1., 1., 0., 0.], &mut || Ok(()))
            .unwrap()
            .unwrap();
        candidate
            .span
            .verify(&design, &candidate.direction)
            .unwrap();
        let mut forged = candidate.direction.clone();
        forged[0] = -1.0;
        assert!(candidate.span.verify(&design, &forged).is_err());
        forged[0] = f64::NAN;
        assert!(candidate.span.verify(&design, &forged).is_err());
        let mut stale = candidate.span.clone();
        stale.split = 1;
        assert!(stale.verify(&design, &candidate.direction).is_err());
        let mut forged = candidate.span.clone();
        forged.potentials[0] += 1;
        assert!(forged.verify(&design, &candidate.direction).is_err());
        let swapped_g = [1, 1, 0, 0, 1, 1];
        let swapped_gi = LevelIndex::from_codes(&swapped_g, 2).unwrap();
        let swapped = BorrowedInterceptFeDesign::new(
            g.len(),
            vec![
                BorrowedInterceptEffect::new(&swapped_g, 2, &swapped_gi).unwrap(),
                BorrowedInterceptEffect::new(&h, 2, &hi).unwrap(),
            ],
        )
        .unwrap();
        assert!(candidate
            .span
            .verify(&swapped, &candidate.direction)
            .is_err());
    }

    #[test]
    fn small_graphs_match_independent_enumeration_of_all_binary_potentials() {
        let g: Vec<_> = (0..9).map(|i| i / 3).collect();
        let h: Vec<_> = (0..9).map(|i| i % 3).collect();
        // Every edge between distinct SCCs admits a binary cut potential. The
        // exhaustive oracle knows neither DSU, traversal nor component labels.
        for pattern in 0..512 {
            let y: Vec<_> = (0..9).map(|i| f64::from(pattern & (1 << i) != 0)).collect();
            let mut strict = vec![false; 9];
            for bits in 0..64 {
                let z: Vec<i32> = (0..9)
                    .map(|i| ((bits >> g[i]) & 1) - ((bits >> (3 + h[i])) & 1))
                    .collect();
                if (0..9).all(|i| if y[i] > 0. { z[i] == 0 } else { z[i] <= 0 }) {
                    for i in 0..9 {
                        strict[i] |= z[i] < 0;
                    }
                }
            }
            let actual = run(&g, &h, &y).unwrap_or_else(|| vec![0.; 9]);
            assert_eq!(
                actual.iter().map(|v| *v < 0.).collect::<Vec<_>>(),
                strict,
                "pattern {pattern}"
            );
            let reverse: Vec<_> = (0..9).rev().collect();
            let gr: Vec<_> = reverse.iter().map(|&i| g[i]).collect();
            let hr: Vec<_> = reverse.iter().map(|&i| h[i]).collect();
            let yr: Vec<_> = reverse.iter().map(|&i| y[i]).collect();
            let reversed = run(&gr, &hr, &yr).unwrap_or_else(|| vec![0.; 9]);
            assert_eq!(
                reversed.iter().rev().map(|v| *v < 0.).collect::<Vec<_>>(),
                strict
            );
        }
    }

    #[test]
    fn disconnected_cycle_and_chain_keep_all_positive_rows_exactly_zero() {
        let z = run(
            &[0, 1, 2, 3, 0, 1, 2, 3],
            &[0, 1, 2, 3, 1, 0, 3, 4],
            &[1., 1., 1., 1., 0., 0., 0., 0.],
        )
        .unwrap();
        assert_eq!(&z[..6], &[0.; 6]);
        assert!(z[6] < 0. && z[7] < 0.);
    }
    #[test]
    fn bounded_star_finds_control_contrast_beyond_first_pair_and_replays_indices() {
        let h = [0, 1, 0, 1, 0, 0, 1, 0];
        let g = [0, 0, 1, 1, 2, 2, 2, 0];
        let x = [0, 0, 1, 1, 1, 0, 0, 0];
        let hi = LevelIndex::from_codes(&h, 2).unwrap();
        let gi = LevelIndex::from_codes(&g, 3).unwrap();
        let xi = LevelIndex::from_codes(&x, 2).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            8,
            vec![
                BorrowedInterceptEffect::new(&h, 2, &hi).unwrap(),
                BorrowedInterceptEffect::new(&g, 3, &gi).unwrap(),
                BorrowedInterceptEffect::new(&x, 2, &xi).unwrap(),
            ],
        )
        .unwrap();
        let candidate = direction(&design, &[1., 1., 1., 1., 1., 0., 0., 0.], &mut || Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(candidate.span.effects, [1, 2]);
        candidate
            .span
            .verify(&design, &candidate.direction)
            .unwrap();
        assert_eq!(
            candidate
                .direction
                .iter()
                .enumerate()
                .filter_map(|(row, &z)| (z < 0.).then_some(row))
                .collect::<Vec<_>>(),
            vec![5, 6]
        );
        let mut stale = candidate.span.clone();
        stale.effects = [0, 2];
        assert!(stale.verify(&design, &candidate.direction).is_err());
    }
}
