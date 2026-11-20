//! Positive-support certificates for original binary design coordinates.
use super::super::compact_model::BinaryDesign;
use super::{
    fallible_filled, positive_basis_with, raw_positive_dual, reserve, ExactNullspace, Work,
};
use crate::error::Result;

mod wide_dual;

fn positive_basis(
    design: &BinaryDesign,
    outcome: &[f64],
    pivots: &[usize],
    work: &mut Work,
) -> Result<Option<(ExactNullspace, Vec<usize>)>> {
    let width = design.ncols();
    let mut row = fallible_filled(width, 0., "unable to reserve binary positive row")?;
    positive_basis_with(width, outcome, work, pivots, |i, space, work| {
        let ones = design.row(i);
        if !work.take(1, width.saturating_add(ones.len())) {
            return Ok(false);
        }
        row.fill(0.);
        for &j in ones {
            row[j as usize] = 1.;
        }
        space.insert(&row)?;
        Ok(true)
    })
}

pub(in crate::ppml_separation::simplex) fn compact_full_positive_rank(
    design: &BinaryDesign,
    outcome: &[f64],
    pivots: &[usize],
    limit: usize,
) -> Result<bool> {
    Ok(positive_basis(design, outcome, pivots, &mut Work(limit))?
        .is_some_and(|(space, _)| space.rank() == design.ncols()))
}

/// Each zero receives baseline mass and a tableau basic row may receive
/// additional mass. The hint selects rows only; exact original moments prove
/// every accepted dual. Positive representatives have unrestricted weights.
pub(in crate::ppml_separation::simplex) fn compact_no_recession(
    design: &BinaryDesign,
    outcome: &[f64],
    boundary: &[usize],
    weights: &[f64],
    pivots: &[usize],
    limit: usize,
    scratch: usize,
) -> Result<bool> {
    let width = design.ncols();
    let k = width as u128;
    let representative_capacity = width.min(outcome.len());
    if k * (k + 2) + representative_capacity as u128 > scratch as u128
        || boundary.len() as u128 > (1u128 << 53)
        || boundary.len() != weights.len()
        || weights.iter().any(|&w| !w.is_finite() || w < 1.)
        || !outcome
            .iter()
            .enumerate()
            .filter_map(|(i, &y)| (y == 0.).then_some(i))
            .eq(boundary.iter().copied())
    {
        return Ok(false);
    }
    let mut work = Work(limit);
    let Some((basis, ids)) = positive_basis(design, outcome, pivots, &mut work)? else {
        return Ok(false);
    };
    if basis.rank() == width {
        return Ok(true);
    }
    let extras = weights.iter().filter(|&&w| w > 1.).count();
    let fixed = k + ids.capacity() as u128 + 2 * extras as u128 + 1;
    if extras > width || fixed + k * (k + 1) > scratch as u128 {
        return Ok(false);
    }
    let mut aggregate = fallible_filled(width, 0., "unable to reserve binary boundary sums")?;
    let mut extra_rows = Vec::new();
    reserve(
        &mut extra_rows,
        extras,
        "unable to reserve compact dual support",
    )?;
    let mut hint = Vec::new();
    reserve(&mut hint, extras + 1, "unable to reserve compact dual hint")?;
    hint.push(-1.);
    for (position, (&i, &w)) in boundary.iter().zip(weights).enumerate() {
        crate::interrupt::row_checkpoint(position)?;
        let ones = design.row(i);
        if !work.take(1, ones.len().saturating_add(1)) {
            return Ok(false);
        }
        for &j in ones {
            aggregate[j as usize] += 1.;
        }
        if w > 1. {
            extra_rows.push(i);
            hint.push(1. - w);
        }
    }
    if quotient_balance(
        design,
        outcome,
        &basis,
        (&aggregate, &extra_rows, &hint),
        &mut work,
        scratch as u128 - fixed,
    )? {
        return Ok(true);
    }
    drop(basis);
    let m = ids.len() + extras + 1;
    let rows = m as u128;
    // Binary searches on each original coordinate can be needed up to five
    // times by the one-prime solver. On a refusal reserve the work it could
    // have consumed before trying fraction-free elimination.
    let lookup = (usize::BITS - width.leading_zeros()) as usize;
    if fixed + rows * (rows + 7) <= scratch as u128
        && work.take(width, m.saturating_mul(5).saturating_mul(lookup))
    {
        let proved = raw_positive_dual(width, m, ids.len(), Some(&hint), 0., work.0, |j, col| {
            let (positive, boundary) = col.split_at_mut(ids.len());
            for (value, &i) in positive.iter_mut().zip(&ids) {
                *value = f64::from(design.row(i).binary_search(&(j as u32)).is_ok());
            }
            boundary[0] = aggregate[j];
            for (value, &i) in boundary[1..].iter_mut().zip(&extra_rows) {
                *value = f64::from(design.row(i).binary_search(&(j as u32)).is_ok());
            }
            Ok(())
        })?
        .0;
        if proved {
            return Ok(true);
        }
        let passes = width
            .saturating_mul(m.min(width) + 1)
            .saturating_add(4usize.saturating_mul(m.saturating_add(width)));
        work.0 = work.0.saturating_sub(passes.saturating_mul(m));
    }
    wide_dual::certify(
        design,
        outcome,
        &ids,
        &extra_rows,
        &aggregate,
        scratch as u128 - fixed,
        &mut work,
    )
}

/// A boundary total belongs to the positive row span exactly when it
/// annihilates the positive right kernel. This avoids reconstructing large
/// rational coefficients on positive observations. Every modular free-column
/// relation must vanish on every positive row, and its nonzero free
/// coordinate establishes independence from all other reconstructed relations.
fn quotient_balance(
    design: &BinaryDesign,
    outcome: &[f64],
    space: &ExactNullspace,
    (aggregate, extras, hint): (&[f64], &[usize], &[f64]),
    work: &mut Work,
    scratch: u128,
) -> Result<bool> {
    let width = design.ncols();
    let dim = width - space.rank();
    let k = width as u128;
    let m = hint.len();
    let equations_size = dim as u128 * m as u128;
    // Positive echelon, its reconstruction, an i128 equation, a potential
    // compressed dual echelon and all its original-coordinate equations.
    let proof_words = (4 * k).max((m as u128) * (m as u128 + 7));
    if k * (k + 1) + 2 * m as u128 + equations_size + proof_words > scratch {
        return Ok(false);
    }
    let mut equations = Vec::new();
    reserve(
        &mut equations,
        equations_size as usize,
        "unable to reserve quotient moments",
    )?;
    let mut free = fallible_filled(width, 0i64, "unable to reserve positive kernel coordinates")?;
    let mut moments = fallible_filled(m, 0i128, "unable to reserve exact quotient row")?;
    let mut uniform = true;
    for j in 0..width {
        if space.pivot(j) {
            continue;
        }
        crate::interrupt::checkpoint()?;
        if !work.take(space.rank() + 3, width) || !work.take(1, outcome.len()) {
            return Ok(false);
        }
        free[j] = 1;
        let Some(relation) = space.reconstruct(&free)? else {
            if crate::diagnostics::solver_trace_enabled() {
                eprintln!("FEREG_SOLVER_TRACE|phase=compact_terminal|failed=kernel_lift|column={j}|extras={}", extras.len());
            }
            return Ok(false);
        };
        free[j] = 0;
        // All visits are subsets of a CSR allocation with at most usize::MAX
        // nonzeros. Even the aggregate of i64 coefficients fits in i128.
        let score = |row: &[u32]| {
            row.iter()
                .map(|&c| i128::from(relation[c as usize]))
                .sum::<i128>()
        };
        for (i, &y) in outcome.iter().enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            if y > 0. {
                let row = design.row(i);
                if !work.take(1, row.len()) || score(row) != 0 {
                    if crate::diagnostics::solver_trace_enabled() {
                        eprintln!("FEREG_SOLVER_TRACE|phase=compact_terminal|failed=kernel_replay|column={j}|extras={}", extras.len());
                    }
                    return Ok(false);
                }
            }
        }
        moments[0] = aggregate
            .iter()
            .zip(&relation)
            .map(|(&count, &v)| i128::from(count as i64) * i128::from(v))
            .sum();
        uniform &= moments[0] == 0;
        for (moment, &i) in moments[1..].iter_mut().zip(extras) {
            let row = design.row(i);
            if !work.take(1, row.len() + 1) {
                return Ok(false);
            }
            *moment = score(row);
        }
        if moments.iter().any(|&v| v != 0) {
            // Conversion to the raw dual's dyadic coordinate type must be exact.
            if moments.iter().any(|v| v.unsigned_abs() > 1u128 << 53) {
                return Ok(false);
            }
            equations.extend(moments.iter().map(|&v| v as f64));
        }
    }
    if uniform {
        return Ok(true);
    }
    drop(free);
    drop(moments);
    let count = equations.len() / m;
    // Reserve the worst-case charged work for one raw dual attempt (one
    // echelon and at most four reconstructions). Leave the rest for the
    // independent raw-coordinate fallback.
    let passes = count
        .saturating_mul(m + 1)
        .saturating_add(4usize.saturating_mul(m.saturating_add(count)));
    let allowance = passes.saturating_mul(m);
    if !work.take(1, allowance) {
        return Ok(false);
    }
    let certified = raw_positive_dual(count, m, 0, Some(hint), 0., allowance, |j, values| {
        values.copy_from_slice(&equations[j * m..(j + 1) * m]);
        Ok(())
    })?
    .0;
    if crate::diagnostics::solver_trace_enabled() {
        eprintln!("FEREG_SOLVER_TRACE|phase=compact_terminal|quotient_equations={count}|extras={}|certified={}", extras.len(), u8::from(certified));
    }
    Ok(certified)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_independent_boundary_contrasts_need_three_distinct_adjustments() {
        let outcome: Vec<_> = (0..18).map(|i| f64::from(i % 6 < 2)).collect();
        let positives: Vec<_> = (0..18).filter(|i| i % 6 < 2).collect();
        let boundary: Vec<_> = (0..18).filter(|i| i % 6 >= 2).collect();
        let weights: Vec<_> = boundary
            .iter()
            .map(|i| if i % 6 == 5 { 3. } else { 1. })
            .collect();
        for closed in [true, false] {
            let design = BinaryDesign::from_rows(18, 12, 1024, |i, row| {
                row.fill(0.);
                let base = 2 * (i / 6);
                let (a, b) = match i % 6 {
                    0 => (0, 0),
                    1 => (1, 1),
                    5 if closed || i < 12 => (1, 0),
                    _ => (0, 1),
                };
                row[base + a] = 1.;
                row[6 + base + b] = 1.;
                Ok(())
            })
            .unwrap()
            .unwrap();
            let mut counts = [0.; 12];
            for &i in &boundary {
                for &j in design.row(i) {
                    counts[j as usize] += 1.;
                }
            }
            let mut work = Work(1_000_000);
            let (space, _) = positive_basis(&design, &outcome, &positives, &mut work)
                .unwrap()
                .unwrap();
            assert_eq!(space.rank(), 6);
            assert_eq!(
                quotient_balance(
                    &design,
                    &outcome,
                    &space,
                    (&counts, &[5, 11, 17], &[-1., -2., -2., -2.]),
                    &mut work,
                    10_000
                )
                .unwrap(),
                closed
            );
            let check = |weights: &[f64], boundary: &[usize], scratch| {
                compact_no_recession(
                    &design, &outcome, boundary, weights, &positives, 1_000_000, scratch,
                )
                .unwrap()
            };
            // In the closed case each positive has weight -3, the three
            // repeated off-diagonal zeros have +1 and the opposite zero +3.
            // Removing that reverse edge leaves a one-way separating ray.
            assert_eq!(check(&weights, &boundary, 10_000), closed);
            assert!(!check(&[1.; 12], &boundary, 10_000));
            assert!(!check(&weights[..11], &boundary[..11], 10_000));
            assert!(!check(&weights, &boundary, 5));
            assert!(!compact_full_positive_rank(&design, &outcome, &positives, 1_000_000).unwrap());
        }
    }
}
