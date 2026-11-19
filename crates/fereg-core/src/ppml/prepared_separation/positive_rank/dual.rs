//! Last-resort exact support search on small generations. Give unrestricted
//! positive-outcome rows pivot priority and try basic boundary solutions.
//! Every boundary row enters the sign and full-design moment checks.
use super::*;

fn dual_words(rows: usize) -> usize {
    rows.saturating_mul(rows.saturating_add(7))
}

pub(in super::super) fn exact_dual(
    method: SeparationMethod,
    model: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    limit: usize,
    config: SeparationConfig,
) -> Result<Option<DetectionProposal>> {
    let n = model.outcome().len();
    let boundary = n - positive;
    // Charge the raw-coordinate reader, including its borrowed slope slices.
    let metadata = topology.effect_specs.iter().fold(0usize, |sum, effect| {
        sum.saturating_add(7)
            .saturating_add(effect.slopes.len().saturating_mul(2))
    });
    let limit = limit.saturating_sub(metadata);
    if positive == 0 || boundary == 0 || dual_words(boundary) > limit {
        return Ok(None);
    }
    let design = primal::RawRows::new(model, true)?;
    // A structural constant can duplicate an explicit or absorbed column.
    // Keeping its equation makes the proof independent of this alias.
    let structural = usize::from(model.fit_view().fit_structural_intercept());
    let columns = design
        .width
        .checked_add(structural)
        .ok_or_else(|| allocation("exact dual width overflow"))?;
    let mut work = primal::ProofBudget::new(model, method, config)
        .work_cells
        .saturating_mul(8)
        .min(32 * 1024 * 1024);
    let Some(mut rows) = dual_rows(
        &design,
        model.outcome(),
        positive,
        columns,
        limit,
        &mut work,
    )?
    else {
        return Ok(None);
    };
    let m = rows.len();
    let positive = m - boundary;
    // The modular echelon and reconstruction, row ids and boundary combination
    // must fit simultaneously. One pass reads each original column twice.
    let peak = dual_words(m)
        .saturating_add(rows.capacity())
        .saturating_add(boundary.min(columns));
    let inner = columns.min(m) as u128 + 2 + 2 * topology.effect_specs.len() as u128;
    let pass = (m as u128).saturating_mul(
        (columns as u128)
            .saturating_mul(inner)
            .saturating_add(m as u128),
    );
    if columns == 0 || peak > limit || pass > work as u128 {
        return Ok(None);
    }
    let mut basis = Vec::new();
    for _ in 0..work / pass as usize {
        for (i, &j) in basis.iter().enumerate() {
            rows.swap(positive + i, positive + j);
        }
        let (proved, pivots) = crate::ppml_separation::raw_positive_dual(
            columns,
            m,
            positive,
            None,
            config.certificate_tolerance,
            pass as usize,
            |j, values| {
                if j == design.width {
                    values.fill(1.);
                } else {
                    for (value, &r) in values.iter_mut().zip(&rows) {
                        *value = design.value(r, j)?;
                    }
                }
                Ok(())
            },
        )?;
        if proved {
            return Ok(Some(no_separation(
                method,
                SeparationCertificateProof::ExactPositiveDual {
                    boundary_rows: boundary,
                },
            )));
        }
        for (i, &j) in basis.iter().enumerate().rev() {
            rows.swap(positive + i, positive + j);
        }
        // A feasible positive dual can be scaled to w >= 1 on the boundary.
        // At a basic feasible solution, the coordinates above one lie in a
        // basis of the boundary rows modulo the unrestricted positive rows.
        // Give each admitted subset pivot priority and propose unit free weights.
        if basis.is_empty() {
            basis = fallible_collect(
                0..pivots,
                pivots,
                "unable to reserve exact dual pivot indices",
            )?;
        }
        if !next_basis(&mut basis, boundary) {
            break;
        }
    }
    Ok(None)
}

fn next_basis(indices: &mut [usize], size: usize) -> bool {
    let count = indices.len();
    if count > size {
        return false;
    }
    let Some(i) = (0..count).rfind(|&i| indices[i] < size - count + i) else {
        return false;
    };
    let next = indices[i] + 1;
    for (offset, index) in indices[i..].iter_mut().enumerate() {
        *index = next + offset;
    }
    true
}

/// Unused positive outcomes get exactly zero weight. If retaining all rows
/// exceeds the budget, select modularly independent positive representatives.
/// Their spanning property is unnecessary: the final raw moments suffice.
fn dual_rows(
    design: &primal::RawRows<'_>,
    outcome: &[f64],
    positive: usize,
    columns: usize,
    limit: usize,
    work: &mut usize,
) -> Result<Option<Vec<usize>>> {
    let n = outcome.len();
    let boundary = n - positive;
    let boundary_ids = || (0..n).filter(|&row| outcome[row] == 0.);
    let positives = (0..n).filter(|&row| outcome[row] > 0.);
    if dual_words(n)
        .saturating_add(n)
        .saturating_add(boundary.min(columns))
        <= limit
    {
        return fallible_collect(
            positives.chain(boundary_ids()),
            n,
            "unable to reserve exact dual row ids",
        )
        .map(Some);
    }
    let capacity = boundary.saturating_add(positive.min(columns));
    if columns
        .saturating_mul(columns.saturating_add(2))
        .saturating_add(capacity)
        > limit
    {
        return Ok(None);
    }
    let mut ids = Vec::new();
    ids.try_reserve_exact(capacity)
        .map_err(|_| allocation("unable to reserve exact dual pivot rows"))?;
    let mut space = exact_arithmetic::ExactNullspace::new(columns)?;
    let mut row = fallible_filled(columns, 1., "unable to reserve exact positive-pivot row")?;
    for r in positives {
        let rank = space.rank();
        let Some(remaining) = work.checked_sub(columns.saturating_mul(rank + 2)) else {
            return Ok(None);
        };
        *work = remaining;
        crate::interrupt::row_checkpoint(r)?;
        design.write(r, &mut row[..design.width])?;
        space.insert(&row)?;
        if space.rank() > rank {
            ids.push(r);
            if space.rank() == columns {
                break;
            }
        }
    }
    ids.extend(boundary_ids());
    Ok(Some(ids))
}
