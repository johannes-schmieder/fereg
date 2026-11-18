//! Terminal simplex claims concern the stored dyadic design. Floating-point
//! elimination and the tableau can propose certificates but cannot prove rank
//! or orthogonality when the boundary cone is small in the regressor's units.

use super::{fallible_filled, reserve};
use crate::data::DenseMatrix;
use crate::error::Result;
use crate::ppml::{exact_dot, ExactNullspace};
use std::cmp::Ordering;

struct Work(usize);

impl Work {
    fn take(&mut self, rows: usize, width: usize) -> bool {
        let Some(remaining) = self.0.checked_sub(rows.saturating_mul(width)) else {
            return false;
        };
        self.0 = remaining;
        true
    }
}

fn positive_basis(
    design: &DenseMatrix,
    outcome: &[f64],
    work: &mut Work,
    pivots: &[usize],
) -> Result<Option<(ExactNullspace, Vec<usize>)>> {
    let k = design.ncols();
    let mut basis = ExactNullspace::new(k)?;
    let mut representatives = Vec::new();
    reserve(
        &mut representatives,
        k.min(outcome.len()),
        "unable to reserve positive pivot rows",
    )?;
    // The floating elimination supplies pivot candidates in its chosen order.
    // Their original rows still require a nonzero modular minor. If a pivot
    // disappears over this prime, other positive rows may supply a substitute.
    let order = pivots.iter().copied().chain(0..outcome.len());
    for (position, i) in order.enumerate() {
        crate::interrupt::row_checkpoint(position)?;
        if outcome[i] > 0. {
            let rank = basis.rank();
            if !work.take(rank + 1, k) {
                return Ok(None);
            }
            let row = &design.as_slice()[i * k..(i + 1) * k];
            basis.insert(row)?;
            if basis.rank() > rank {
                representatives.push(i);
                if basis.rank() >= pivots.len() {
                    break;
                }
            }
        }
    }
    Ok(Some((basis, representatives)))
}

/// Changing the units of one column can make its entire boundary cone smaller
/// than the tableau tolerance. Test raw signs before entering phase I.
pub(super) fn single_coordinate(design: &DenseMatrix, y: &[f64]) -> Result<Option<Vec<f64>>> {
    let k = design.ncols();
    for j in 0..k {
        crate::interrupt::checkpoint()?;
        let mut sign = 0.;
        let mut valid = true;
        for (i, &outcome) in y.iter().enumerate() {
            crate::interrupt::row_checkpoint(i)?;
            let value = design[(i, j)];
            if value == 0. {
                continue;
            }
            if outcome > 0. || !value.is_finite() || (sign != 0. && sign != value.signum()) {
                valid = false;
                break;
            }
            sign = value.signum();
        }
        if valid && sign != 0. {
            let mut gamma = fallible_filled(k, 0., "unable to reserve exact coordinate direction")?;
            gamma[j] = sign;
            return Ok(Some(gamma));
        }
    }
    Ok(None)
}

pub(super) fn full_positive_rank(
    design: &DenseMatrix,
    y: &[f64],
    pivots: &[usize],
    work: usize,
) -> Result<bool> {
    Ok(positive_basis(design, y, &mut Work(work), pivots)?
        .is_some_and(|(basis, _)| basis.rank() == design.ncols()))
}

pub(super) fn no_recession(
    design: &DenseMatrix,
    y: &[f64],
    boundary_dual: &[f64],
    limit: usize,
    scratch_words: usize,
    pivot_rows: &[usize],
) -> Result<bool> {
    let mut work = Work(limit);
    let Some((basis, mut ids)) = positive_basis(design, y, &mut work, pivot_rows)? else {
        return Ok(false);
    };
    let k = design.ncols();
    // Independent real null vectors attaining this modular nullity prove the
    // positive and complete row spaces coincide, including structural aliases.
    if contains_full_design(design, &basis, &mut work)? {
        return Ok(true);
    }
    drop(basis);

    // A raw dual supported on independent positive rows and every zero row
    // suffices. Its positive-outcome weights may have either sign. Require
    // strictly positive boundary weights and exact orthogonality in every
    // original model coordinate. A single bad prime can only decline a proof.
    let positive = ids.len();
    let rows = positive + boundary_dual.len();
    if (rows as u128) * (rows as u128 + 8) + k as u128 + 2 * boundary_dual.len() as u128
        > scratch_words as u128
    {
        return Ok(false);
    }
    reserve(
        &mut ids,
        boundary_dual.len(),
        "unable to reserve exact dual rows",
    )?;
    ids.extend(
        y.iter()
            .enumerate()
            .filter_map(|(row, &outcome)| (outcome == 0.).then_some(row)),
    );
    let mut space = ExactNullspace::new(rows)?;
    let mut column = fallible_filled(rows, 0., "unable to reserve exact dual column")?;
    for j in 0..k {
        if !work.take(space.rank() + 1, rows) {
            return Ok(false);
        }
        crate::interrupt::checkpoint()?;
        for (&r, value) in ids.iter().zip(&mut column) {
            *value = design[(r, j)];
        }
        space.insert(&column)?;
    }
    let maximum = boundary_dual.iter().map(|v| v.abs()).fold(0., f64::max);
    let mut free = fallible_filled(rows, 0i64, "unable to reserve raw dual proposal")?;
    for resolution in [0., 1024., 1_048_576., 4_294_967_296.] {
        if !work.take(rows, rows) || !work.take(k, rows) {
            return Ok(false);
        }
        // Rounding only proposes free-coordinate ratios. Every sign and
        // identity below is checked after exact reconstruction.
        for (value, &multiplier) in free[positive..].iter_mut().zip(boundary_dual) {
            *value = if resolution == 0. {
                1
            } else {
                (-multiplier / maximum * resolution).round().max(1.) as i64
            };
        }
        let Some(dual) = space.reconstruct(&free)? else {
            continue;
        };
        if dual[positive..].iter().any(|&value| value <= 0) {
            continue;
        }
        let mut valid = true;
        for j in 0..k {
            crate::interrupt::checkpoint()?;
            for (&r, value) in ids.iter().zip(&mut column) {
                *value = design[(r, j)];
            }
            if exact_dot(&column, &dual) != Some(Ordering::Equal) {
                valid = false;
                break;
            }
        }
        if valid {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Reconstruct dependencies sparsely. Most redundant categorical levels and
/// repeated regressors involve only a few nonzero coordinates. Verifying their
/// dyadic row identities costs one visit per participating source.
fn contains_full_design(
    design: &DenseMatrix,
    basis: &ExactNullspace,
    work: &mut Work,
) -> Result<bool> {
    let k = design.ncols();
    let mut free = fallible_filled(k, 0i64, "unable to reserve exact span coordinates")?;
    for j in 0..k {
        if basis.pivot(j) {
            continue;
        }
        if !work.take(basis.rank() + 1, k) {
            return Ok(false);
        }
        free[j] = 1;
        let Some(relation) = basis.reconstruct(&free)? else {
            return Ok(false);
        };
        free[j] = 0;
        let support = relation.iter().filter(|&&b| b != 0).count();
        if !work.take(design.nrows(), support) {
            return Ok(false);
        }
        let mut terms = Vec::new();
        let mut coefficients = Vec::new();
        reserve(
            &mut terms,
            support,
            "unable to reserve exact dependency indices",
        )?;
        reserve(
            &mut coefficients,
            support,
            "unable to reserve exact dependency coefficients",
        )?;
        for (j, &b) in relation.iter().enumerate() {
            if b != 0 {
                terms.push(j);
                coefficients.push(b);
            }
        }
        let mut values = fallible_filled(
            support,
            0.,
            "unable to reserve exact dependency coordinates",
        )?;
        for (r, row) in design.as_slice().chunks_exact(k).enumerate() {
            crate::interrupt::row_checkpoint(r)?;
            for (value, &col) in values.iter_mut().zip(&terms) {
                *value = row[col];
            }
            if exact_dot(&values, &coefficients) != Some(Ordering::Equal) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
