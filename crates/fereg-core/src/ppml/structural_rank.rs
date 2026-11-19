//! Exact final-sample rank when the pairwise FE count would reject a PPML fit.
use super::prepared_model::PreparedPpmlModelGeneration;
use super::prepared_separation::exact_arithmetic::{exact_dot, ExactNullspace};
use crate::error::{invalid_argument, Result};
use crate::memory::{fallible_empty, fallible_filled};
use crate::ols::topology_dof::CategoricalTopologyDofReport;
use std::cmp::Ordering;

pub(super) const MAX_BYTES: u128 = 1 << 20;
const MAX_WORK: u128 = 4_000_000;

/// One echelon square, its work vector, the design row, free coordinates,
/// three reconstruction vectors, and a borrowed code slice per channel.
pub(super) fn workspace_bound(width: u128, channels: u128) -> u128 {
    8u128
        .saturating_mul(width)
        .saturating_mul(width.saturating_add(6))
        .saturating_add(channels.saturating_mul(std::mem::size_of::<&[u32]>() as u128))
}

pub(super) fn refine(
    generation: &PreparedPpmlModelGeneration,
    report: &mut CategoricalTopologyDofReport,
    finite_rank: usize,
    admitted_bytes: u128,
) -> Result<()> {
    let effective = generation.weight_report().effective_observations;
    if !report.is_approximate
        || report.absorbed_rank.saturating_add(finite_rank) < effective
        || generation.slope_recovery.is_some()
        || report
            .effects
            .iter()
            .any(|e| e.nested || e.slope_source.is_some())
    {
        return Ok(());
    }
    let data = generation.fit_view();
    let rows = generation.outcome().len();
    // The first partition spans the constant. In every later partition its
    // final indicator is one minus all the others, so removing it preserves
    // the span of every declared prefix before any numerical work.
    let block_width = |channel: usize, levels: usize| levels - usize::from(channel > 0);
    let fe_width = report.initial_categories - (report.effects.len() - 1);
    let width = fe_width.saturating_add(data.model().columns.len());
    let w = width as u128;
    let channels = report.effects.len() as u128;
    // Each exact dependency re-reads the original design. Count every FE
    // channel, including one-level partitions with no encoded columns.
    let row_reads = (rows as u128)
        .saturating_mul(w + 1)
        .saturating_mul(w + channels);
    let elimination = (rows as u128 + w).saturating_mul(w).saturating_mul(w);
    if workspace_bound(w, channels) > admitted_bytes.min(MAX_BYTES)
        || elimination.saturating_add(row_reads) > MAX_WORK
    {
        return Ok(());
    }

    let mut codes = fallible_empty(
        report.effects.len(),
        "unable to reserve structural rank FE views",
    )?;
    for &source in data.fe_topology().effect_to_topology() {
        let effect = crate::fe::ValidatedBorrowedInterceptSource::from_finalized_source(
            &data.fe_topology,
            source,
        )
        .ok_or_else(|| invalid_argument("missing final FE codes for structural rank"))?;
        codes.push(effect.effect().codes());
    }

    // Put the FE indicators first, in declared channel order. The exact pivots
    // then give every prefix rank. Include *all* generated finite columns to
    // verify their within rank independently, including any omitted directions.
    let Some(basis) = certified_basis(rows, width, |i, row| {
        row[..fe_width].fill(0.);
        let mut start = 0;
        for (channel, (codes, effect)) in codes.iter().zip(&report.effects).enumerate() {
            let columns = block_width(channel, effect.categories);
            let level = codes[i] as usize;
            if level < columns {
                row[start + level] = 1.;
            }
            start += columns;
        }
        for (j, value) in row[start..].iter_mut().enumerate() {
            *value = data.column_value(j, i)?;
        }
        Ok(())
    })?
    else {
        return Ok(());
    };
    let absorbed = (0..fe_width).filter(|&j| basis.pivot(j)).count();
    if basis.rank() >= effective || basis.rank() - absorbed != finite_rank {
        return Ok(());
    }
    let mut start = 0;
    for (channel, effect) in report.effects.iter_mut().enumerate() {
        let end = start + block_width(channel, effect.categories);
        effect.coefficients = (start..end).filter(|&j| basis.pivot(j)).count();
        effect.redundant = effect.categories - effect.coefficients;
        effect.redundancy_exact = true;
        start = end;
    }
    report.absorbed_rank = absorbed;
    report.redundant_categories = report.initial_categories - absorbed;
    report.is_approximate = false;
    Ok(())
}

/// A nonzero modular minor proves independence over the rationals. For each
/// remaining column prove an integer relation on the raw dyadics involving
/// only that column and its predecessors. Together these prove every prefix
/// rank. A modular underestimate fails the exact dependency check.
fn certified_basis(
    rows: usize,
    width: usize,
    mut fill: impl FnMut(usize, &mut [f64]) -> Result<()>,
) -> Result<Option<ExactNullspace>> {
    let mut basis = ExactNullspace::new(width)?;
    let mut row = fallible_filled(width, 0., "unable to reserve structural rank row")?;
    for i in 0..rows {
        crate::interrupt::row_checkpoint(i)?;
        fill(i, &mut row)?;
        basis.insert(&row)?;
    }
    let mut free = fallible_filled(
        width,
        0i64,
        "unable to reserve structural rank free coordinates",
    )?;
    for j in 0..width {
        if basis.pivot(j) {
            continue;
        }
        free[j] = 1;
        let Some(relation) = basis.reconstruct(&free)? else {
            return Ok(None);
        };
        free[j] = 0;
        if relation[j] == 0 || relation[j + 1..].iter().any(|&c| c != 0) {
            return Ok(None);
        }
        for i in 0..rows {
            crate::interrupt::row_checkpoint(i)?;
            fill(i, &mut row)?;
            if exact_dot(&row, &relation) != Some(Ordering::Equal) {
                return Ok(None);
            }
        }
    }
    Ok(Some(basis))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ppml::modular_rank::PRIME;

    #[test]
    fn certifies_prefix_ranks_only_after_raw_column_replay() {
        let certify = |a: [[f64; 4]; 3]| {
            certified_basis(3, 4, |i, row| {
                row.copy_from_slice(&a[i]);
                Ok(())
            })
            .unwrap()
        };
        let family = |tiny| [[1., 2., 0., 2.], [0., 0., 1., -4.], [0., tiny, 0., 0.]];
        let basis = certify(family(0.)).unwrap();
        assert_eq!(
            (0..4).map(|j| basis.pivot(j)).collect::<Vec<_>>(),
            [true, false, true, false]
        );
        assert_eq!(certify(family(f64::from_bits(1))).unwrap().rank(), 3);
        // This independent direction vanishes modulo the prime. Its proposed
        // relation fails on row three: a modular underestimate cannot rescue df.
        assert!(certify(family(PRIME as f64)).is_none());
    }
}
