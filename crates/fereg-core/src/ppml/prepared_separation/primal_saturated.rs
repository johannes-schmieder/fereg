//! Exact interpolation on saturated sets of raw design rows. If the k
//! distinct nonzero design patterns have modular row rank k, the same minor
//! is nonsingular over the rationals. Arbitrary scores on those patterns
//! can therefore be interpolated simultaneously by one coefficient vector.

use super::exact_arithmetic::{exact_dot, CellRelations, ExactNullspace};
use crate::error::{allocation, Result};
use crate::memory::fallible_filled;

mod integer;

struct Pattern {
    hash: u64,
    row: usize,
    selected: bool,
}

const PATTERN_WORDS: usize = std::mem::size_of::<Pattern>().div_ceil(8);

/// Try the scores -1 on the proposed rows and 0 on every other observation.
/// The hash indexes representatives only. Raw coordinate equality and target
/// equality are always checked before one observation can stand for another.
/// Peak storage is width² + 3·width eight-byte words and width Patterns.
pub(super) fn supports_target(
    rows: usize,
    width: usize,
    mut fill: impl FnMut(usize, &mut [f64]) -> Result<()>,
    selected: impl Fn(usize) -> bool,
    mut work: usize,
) -> Result<bool> {
    let mut basis = ExactNullspace::new(width)?;
    let patterns = collect_patterns(
        rows,
        width,
        width,
        &mut work,
        &mut fill,
        selected,
        |row, count, remaining| {
            if !charge(remaining, width as u128 * (count as u128 + 3)) {
                return Ok(false);
            }
            basis.insert(row)?;
            Ok(basis.rank() > count)
        },
    )?;
    Ok(patterns.is_some())
}

/// When cell scores are constrained by row dependencies, prove that the
/// proposed score is orthogonal to the whole exact left nullspace. The rank
/// over the prime is a lower bound on the rational rank. Verifying one
/// independent rational relation per modular free coordinate attains the
/// corresponding upper bound, so these relations span every dependency.
pub(super) fn supports_dependent_target_with_hint(
    rows: usize,
    width: usize,
    (limit_words, mut limit_work): (usize, usize),
    mut fill: impl FnMut(usize, &mut [f64]) -> Result<()>,
    selected: impl Fn(usize) -> bool,
    mut value: impl FnMut(usize, usize) -> Result<f64>,
    hint: Option<&[f64]>,
) -> Result<bool> {
    if hint.is_some_and(|values| values.len() != rows) {
        return Ok(false);
    }
    let Some(remaining) = limit_words.checked_sub(width.saturating_mul(2)) else {
        return Ok(false);
    };
    let cap = rows.min(limit_words.isqrt()).min(remaining / PATTERN_WORDS);
    let Some(patterns) = collect_patterns(
        rows,
        width,
        cap,
        &mut limit_work,
        &mut fill,
        selected,
        |_, _, _| Ok(true),
    )?
    else {
        return Ok(false);
    };
    let cells = patterns.len();
    let needed =
        (cells as u128) * (cells as u128 + 6) + PATTERN_WORDS as u128 * patterns.capacity() as u128;
    if needed > limit_words as u128 {
        trace_cell_proof(cells, width, 0, "admission_memory");
        return Ok(false);
    }
    if !charge(
        &mut limit_work,
        width as u128 * cells as u128 * (cells as u128 + 2),
    ) {
        trace_cell_proof(cells, width, 0, "admission_work");
        return Ok(false);
    }
    let mut basis = ExactNullspace::new(cells)?;
    let mut column = fallible_filled(cells, 0., "unable to reserve exact cell column")?;
    let mut load_column = |j, column: &mut [f64]| -> Result<()> {
        crate::interrupt::checkpoint()?;
        for (i, (cell, x)) in patterns.iter().zip(column).enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            *x = value(cell.row, j)?;
        }
        Ok(())
    };
    for j in 0..width {
        load_column(j, &mut column)?;
        basis.insert(&column)?;
        if basis.rank() == cells {
            return Ok(true);
        }
    }
    let rank = basis.rank();
    let nullity = (cells - rank) as u128;
    let targets = patterns.iter().filter(|p| p.selected).count() as u128;
    let relation_work = nullity * cells as u128 * (rank as u128 + width as u128 + 2)
        + nullity * targets * targets
        + 5 * (targets * targets + nullity * cells as u128);
    if !charge(&mut limit_work, relation_work) {
        trace_cell_proof(cells, width, rank, "work");
        return Ok(false);
    }
    let Some(relations) = basis.into_relations()? else {
        trace_cell_proof(cells, width, rank, "reconstruct");
        drop(column);
        let available = limit_words - PATTERN_WORDS * patterns.capacity();
        return integer::certify(
            &patterns,
            width,
            rank,
            value,
            hint,
            (available, &mut limit_work),
        );
    };
    let mut scratch = fallible_filled(cells, 0i64, "unable to reserve exact cell relation")?;
    // Validate all proposed relations together. Each source column is read
    // only twice regardless of the number of relations.
    for j in 0..width {
        load_column(j, &mut column)?;
        if !relations.all(&mut scratch, |w| {
            Ok(exact_dot(&column, w) == Some(std::cmp::Ordering::Equal))
        })? {
            trace_cell_proof(cells, width, rank, "raw_relation");
            return Ok(false);
        }
    }
    drop(column);
    let used =
        cells as u128 * (cells as u128 + 2) + PATTERN_WORDS as u128 * patterns.capacity() as u128;
    let proved = target_in_span(
        &relations,
        &patterns,
        &mut scratch,
        limit_words as u128 - used,
        hint,
    )?;
    trace_cell_proof(
        cells,
        width,
        rank,
        if proved { "verified" } else { "target" },
    );
    Ok(proved)
}

fn trace_cell_proof(cells: usize, width: usize, rank: usize, stage: &str) {
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!("FEREG_SOLVER_TRACE|phase=separation_cell_relations|stage={stage}|cells={cells}|width={width}|rank={rank}"));
    }
}

fn charge(remaining: &mut usize, amount: u128) -> bool {
    if amount > *remaining as u128 {
        return false;
    }
    *remaining -= amount as usize;
    true
}

/// Target scores may differ. Zero is prescribed on unselected cells; the
/// selected subvector must be strictly negative and orthogonal to each
/// previously verified relation. A candidate is checked over the integers.
fn target_in_span(
    relations: &CellRelations,
    patterns: &[Pattern],
    scratch: &mut [i64],
    available: u128,
    hint: Option<&[f64]>,
) -> Result<bool> {
    let selected = || patterns.iter().enumerate().filter(|(_, p)| p.selected);
    if relations.all(scratch, |w| {
        Ok(selected().map(|(i, _)| i128::from(w[i])).sum::<i128>() == 0)
    })? {
        return Ok(true);
    }
    let size = selected().count();
    if size as u128 * (size as u128 + 6) > available || size == 1 {
        return Ok(false);
    }
    let mut scores = ExactNullspace::new(size)?;
    let mut free = fallible_filled(size, -1i64, "unable to reserve separated cell scores")?;
    relations.all(scratch, |w| {
        for (dest, (i, _)) in free.iter_mut().zip(selected()) {
            *dest = w[i];
        }
        scores.insert_integers(&free)?;
        Ok(true)
    })?;
    for scale in [1., 16., 256., 4096., 32_767.] {
        free.fill(-1);
        if scale > 1. {
            let Some(h) = hint else { break };
            let maximum = selected().map(|(_, p)| -h[p.row]).fold(0., f64::max);
            if !(maximum.is_finite() && maximum > 0.) {
                break;
            }
            for (weight, (_, p)) in free.iter_mut().zip(selected()) {
                *weight = -((-h[p.row] / maximum * scale).round().max(1.) as i64);
            }
        }
        let Some(candidate) = scores.reconstruct(&free)? else {
            continue;
        };
        if candidate.iter().all(|&v| v < 0)
            && relations.all(scratch, |w| {
                Ok(selected()
                    .zip(&candidate)
                    .try_fold(0i128, |sum, ((i, _), &v)| {
                        sum.checked_add(i128::from(v) * i128::from(w[i]))
                    })
                    == Some(0))
            })?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn collect_patterns(
    rows: usize,
    width: usize,
    capacity: usize,
    remaining: &mut usize,
    fill: &mut impl FnMut(usize, &mut [f64]) -> Result<()>,
    selected: impl Fn(usize) -> bool,
    mut accept_new: impl FnMut(&[f64], usize, &mut usize) -> Result<bool>,
) -> Result<Option<Vec<Pattern>>> {
    let mut row = fallible_filled(width, 0., "unable to reserve saturated separation row")?;
    let mut previous = fallible_filled(width, 0., "unable to reserve separation representative")?;
    let mut patterns: Vec<Pattern> = Vec::new();
    patterns
        .try_reserve_exact(capacity)
        .map_err(|_| allocation("unable to reserve saturated separation patterns"))?;
    let mut has_target = false;
    for index in 0..rows {
        crate::interrupt::row_checkpoint(index)?;
        if !charge(remaining, 2 * width as u128 + usize::BITS as u128) {
            return Ok(None);
        }
        fill(index, &mut row)?;
        let target = selected(index);
        has_target |= target;
        if row.iter().all(|&x| x == 0.) {
            if target {
                return Ok(None);
            }
            continue;
        }
        if row.iter().any(|x| !x.is_finite()) {
            return Ok(None);
        }
        // Both signed zeros denote the same rational coordinate.
        let hash = row.iter().fold(0xcbf29ce484222325u64, |h, &x| {
            (h ^ if x == 0. { 0 } else { x.to_bits() }).wrapping_mul(0x100000001b3)
        });
        let start = patterns.partition_point(|p| p.hash < hash);
        let mut found = false;
        for pattern in patterns[start..].iter().take_while(|p| p.hash == hash) {
            if !charge(remaining, 2 * width as u128) {
                return Ok(None);
            }
            fill(pattern.row, &mut previous)?;
            if row == previous {
                if target != pattern.selected {
                    return Ok(None);
                }
                found = true;
                break;
            }
        }
        if found {
            continue;
        }
        if patterns.len() == capacity
            || !charge(remaining, patterns.len() as u128)
            || !accept_new(&row, patterns.len(), remaining)?
        {
            return Ok(None);
        }
        patterns.insert(
            start,
            Pattern {
                hash,
                row: index,
                selected: target,
            },
        );
    }
    Ok(has_target.then_some(patterns))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check<const P: usize>(data: &[[f64; P]], targets: &[bool]) -> bool {
        supports_target(
            data.len(),
            P,
            |i, row| {
                row.copy_from_slice(&data[i]);
                Ok(())
            },
            |i| targets[i],
            usize::MAX,
        )
        .unwrap()
    }

    #[test]
    fn large_exact_determinant_and_repeated_patterns_interpolate() {
        // The determinant exceeds the symmetric rational-reconstruction
        // bound; nonvanishing modulo the prime still certifies interpolation.
        let data = [[1., 0.], [1., 83_886_071.], [1., -0.], [1., 83_886_071.]];
        assert!(check(&data, &[false, true, false, true]));
        assert!(!check(&data, &[false, true, true, true]));
    }

    #[test]
    fn dependence_and_a_strict_score_on_a_zero_row_are_inconclusive() {
        let data = [[1., 0.], [0., 1.], [1., 1.]];
        assert!(!check(&data, &[false, false, true]));
        let zero = [[0., -0.], [1., 1.]];
        assert!(check(&zero, &[false, true]));
        assert!(!check(&zero, &[true, false]));
    }

    fn check_relations<const P: usize>(data: &[[f64; P]], targets: &[bool], limit: usize) -> bool {
        supports_dependent_target_with_hint(
            data.len(),
            P,
            (limit, usize::MAX),
            |i, row| {
                row.copy_from_slice(&data[i]);
                Ok(())
            },
            |i| targets[i],
            |i, j| Ok(data[i][j]),
            None,
        )
        .unwrap()
    }

    #[test]
    fn exact_cell_dependencies_allow_consistent_targets_only() {
        let cells = [[1., 0.], [0., 1.], [1., 1.], [1., -0.]];
        assert!(check_relations(&cells, &[true, false, true, true], 128));
        assert!(!check_relations(&cells, &[true, false, true, false], 128));
        assert!(!check_relations(&cells, &[false, false, true, false], 128));
        assert!(!check_relations(&cells, &[true, false, true, true], 10));
    }

    #[test]
    fn relation_valid_only_modulo_the_prime_cannot_certify_equal_cell_scores() {
        let p = crate::ppml::modular_rank::PRIME as f64;
        assert!(!check_relations(&[[1.], [1. + p]], &[true, true], 64));
    }

    #[test]
    fn strictly_negative_scores_can_have_unequal_magnitudes() {
        assert!(check_relations(
            &[[1., 0.], [0., 1.], [2., 1.]],
            &[true, false, true],
            128
        ));
        assert!(!check_relations(
            &[[1., 0.], [0., 1.], [-2., 1.]],
            &[true, false, true],
            128
        ));
    }
}
