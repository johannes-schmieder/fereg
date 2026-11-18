//! A saturated categorical design has a zero total weighted residual in each
//! design cell. If the cells lie in a common coarsest cluster, its group scores
//! vanish (including the private intercept). Every other nonempty CGM subset
//! has the same partition as its intersection with that cluster and the opposite
//! inclusion–exclusion sign. Thus the complete meat is exactly zero.

use super::modular_rank::insert;
use super::prepared_model::PreparedPpmlModelGeneration;
use crate::error::Result;
use crate::memory::fallible_filled;
use crate::ols::prepared_fit_view::visit_prepared_column_range;
use crate::ols::PreparedOlsColumnKind;

/// Bound optional proof work independently of whether the data satisfy it.
/// Its scratch replaces the already-admitted clustered meat workspace.
const MAX_FIELD_OPERATIONS: u128 = 32 << 20;

pub(super) fn certified(
    generation: &PreparedPpmlModelGeneration,
    kept: &[usize],
    clusters: &[&[u32]],
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<bool> {
    let data = generation.fit_view();
    if clusters.is_empty()
        || kept.is_empty()
        || data.has_absorbed_slope()
        || kept.iter().any(|&j| {
            matches!(
                data.model().columns[j].kind,
                PreparedOlsColumnKind::Continuous { .. }
            )
        })
    {
        return Ok(false);
    }
    let registry = data.fe_topology().registry();
    let Some(width) = kept.len().checked_add(registry.total_levels()?) else {
        return Ok(false);
    };
    let rows = generation.outcome().len();
    let n = rows as u128;
    let k = width as u128;
    if n.saturating_mul(k).saturating_mul(k.min(n)) > MAX_FIELD_OPERATIONS {
        return Ok(false);
    }
    let bytes = n * (k + std::mem::size_of::<usize>() as u128) + 8 * k * (k + 1);
    // The final memory plan admits the generated finite width before rank
    // omission. Use this same scratch bank, before any score reduction starts.
    let admitted_columns = data.model().columns.len();
    if bytes > super::vce::clustered_score_workspace_bound(rows, admitted_columns)? {
        return Ok(false);
    }
    let dimensions = data.clusters().dimensions();
    let coarse = dimensions
        .iter()
        .enumerate()
        .min_by_key(|(_, dim)| dim.groups())
        .unwrap()
        .0;
    // Every coarse group must contain a whole cell. More such groups than
    // available columns rules out saturation without reading the rows.
    if dimensions[coarse].groups() > width {
        return Ok(false);
    }
    for (j, fine) in clusters.iter().enumerate() {
        if j != coarse && !nested(fine, dimensions[j].groups(), clusters[coarse])? {
            return Ok(false);
        }
    }
    let mut design = fallible_filled(
        rows * width,
        0_u8,
        "unable to reserve PPML categorical design cells",
    )?;
    for (column, &generated) in kept.iter().enumerate() {
        poll()?;
        visit_prepared_column_range(data, generated, 0..rows, |row, indicator| {
            design[row * width + column] = u8::from(indicator == 1.0);
            Ok(())
        })?;
    }
    // Registry sources are the distinct absorbed partitions. Each term has an
    // intercept here, since terms with continuous absorbed slopes were declined.
    let mut column = kept.len();
    for effect in registry.sources() {
        for level in 0..effect.n_levels() {
            poll()?;
            for &row in effect.members(level)? {
                design[row * width + column] = 1;
            }
            column += 1;
        }
    }
    saturated_cells(&design, width, generation.outcome(), clusters[coarse], poll)
}

fn nested(fine: &[u32], groups: usize, coarse: &[u32]) -> Result<bool> {
    let mut owner = fallible_filled(
        groups,
        u32::MAX,
        "unable to reserve PPML cluster nesting map",
    )?;
    for (row, (&group, &parent)) in fine.iter().zip(coarse).enumerate() {
        crate::interrupt::row_checkpoint(row)?;
        let slot = &mut owner[group as usize];
        if *slot != u32::MAX && *slot != parent {
            return Ok(false);
        }
        *slot = parent;
    }
    Ok(true)
}

fn saturated_cells(
    design: &[u8],
    width: usize,
    outcome: &[f64],
    coarse: &[u32],
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<bool> {
    let recipe = |row: usize| &design[row * width..(row + 1) * width];
    let mut order = fallible_filled(
        outcome.len(),
        0usize,
        "unable to reserve PPML design-cell order",
    )?;
    for (id, row) in order.iter_mut().enumerate() {
        *row = id;
    }
    order.sort_unstable_by(|&a, &b| recipe(a).cmp(recipe(b)));
    let mut cells = 0;
    let mut start = 0;
    while start < order.len() {
        let representative = order[start];
        let mut end = start;
        let mut positive = false;
        while end < order.len() && recipe(order[end]) == recipe(representative) {
            crate::interrupt::row_checkpoint(end)?;
            if coarse[order[end]] != coarse[representative] {
                return Ok(false);
            }
            positive |= outcome[order[end]] > 0.0;
            end += 1;
        }
        // A cell with only zero outcomes has no finite saturated optimum.
        if !positive || cells == width {
            return Ok(false);
        }
        order[cells] = representative;
        cells += 1;
        start = end;
    }
    let mut basis = fallible_filled(
        width * width,
        0u64,
        "unable to reserve PPML saturated rank basis",
    )?;
    let mut row = fallible_filled(width, 0u64, "unable to reserve PPML saturated rank row")?;
    for &id in &order[..cells] {
        poll()?;
        for (dst, &bit) in row.iter_mut().zip(recipe(id)) {
            *dst = u64::from(bit);
        }
        // Independent cell rows over the prime field prove independence over R.
        if !insert(&mut basis, &mut row) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturated_cells_require_independent_rows_positive_support_and_cluster_ownership() {
        let run =
            |x: &[u8], y: &[f64], g: &[u32]| saturated_cells(x, 3, y, g, &mut || Ok(())).unwrap();
        // A determinant of two is a valid witness, including when rows repeat.
        let design = [1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1];
        let y = [1., 0., 1., 2.];
        assert!(run(&design, &y, &[0, 1, 0, 1]));
        assert!(!run(&design, &y, &[0, 1, 0, 0]));
        assert!(!run(&design, &[0., 1., 1., 1.], &[0, 1, 0, 1]));
        assert!(!run(&[1, 0, 0, 0, 1, 0, 1, 1, 0], &[1.; 3], &[0, 0, 1]));
        assert!(nested(&[2, 1, 2, 0], 3, &[0, 1, 0, 1]).unwrap());
        assert!(!nested(&[2, 1, 2, 0], 3, &[0, 1, 1, 1]).unwrap());
    }
}
