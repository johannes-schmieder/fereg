//! An exact positive-support rank proof for a single categorical intercept.
//! Within each FE level, subtract one positive row from all other positive
//! rows. If these differences span the finite regressors, a direction zero
//! on the positives has zero slope. One positive per level then forces its
//! absorbed coefficients to zero as well. A modular rank deficit proves
//! nothing, so an inconclusive scan leaves the ordinary detector in charge.
use super::*;

use crate::ppml::modular_rank::{insert, residue, PRIME};

pub(super) fn single_intercept(
    generation: &PreparedPpmlModelGeneration,
    topology: &DetectorTopology<'_>,
    positive: usize,
    topology_cells: usize,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<DetectionProposal>> {
    let [effect] = topology.effect_specs else {
        return Ok(None);
    };
    if !effect.intercept || !effect.slopes.is_empty() {
        return Ok(None);
    }
    let levels = topology.sources[topology.effect_to_source[0]];
    let view = generation.fit_view();
    let columns = &view.model().columns;
    let slopes = || {
        columns.iter().enumerate().filter_map(|(i, c)| {
            (!matches!(c.kind, crate::ols::PreparedOlsColumnKind::Intercept)).then_some(i)
        })
    };
    let width = slopes().count();
    let Some(full_rank) = width.checked_add(levels.n_levels()) else {
        return Ok(None);
    };
    let limit = config
        .simplex_max_tableau_cells
        .saturating_sub(topology_cells);
    if width == 0 || positive < full_rank || width.saturating_mul(width.saturating_add(2)) > limit {
        return Ok(None);
    }
    let mut basis = fallible_filled(
        width * width,
        0u64,
        "unable to reserve positive-row rank basis",
    )?;
    let mut anchor = fallible_filled(width, 0u64, "unable to reserve positive-row anchor")?;
    let mut row = fallible_filled(width, 0u64, "unable to reserve positive-row rank work row")?;
    let mut rank = 0;
    // Bound unsuccessful work on aliased or sparse designs. Discovering a
    // witness sooner is sufficient; reaching this budget gives no conclusion.
    let attempts_limit = config
        .simplex_max_iterations
        .min(config.simplex_max_tableau_cells.saturating_mul(8) / (width * width));
    let mut attempts = 0;
    for level in 0..levels.n_levels() {
        interrupt(PpmlInterruptPoint::SeparationSimplex)?;
        let members = levels.checked_members(level)?;
        let mut found = false;
        for (position, &index) in members.iter().enumerate() {
            crate::interrupt::row_checkpoint(position)?;
            if generation.outcome()[index] <= 0.0 {
                continue;
            }
            if rank == width {
                found = true;
                break;
            }
            if !found {
                for (dest, column) in anchor.iter_mut().zip(slopes()) {
                    *dest = residue(view.column_value(column, index)?);
                }
                found = true;
                continue;
            }
            if attempts >= attempts_limit {
                return Ok(None);
            }
            attempts += 1;
            for ((dest, &origin), column) in row.iter_mut().zip(&anchor).zip(slopes()) {
                *dest = (residue(view.column_value(column, index)?) + PRIME - origin) % PRIME;
            }
            rank += usize::from(insert(&mut basis, &mut row));
            if rank == width {
                break;
            }
        }
        if !found {
            return Ok(None);
        }
    }
    if rank != width {
        return Ok(None);
    }
    let mut result = DetectionProposal::empty(
        SeparationMethod::Simplex,
        MethodStatus::NoSeparation,
        "exact positive-outcome rank spans the finite regressors and absorbed intercept levels",
    );
    result.certificate = Some(crate::ppml_separation::no_separation_certificate(
        crate::ppml_separation::SeparationCertificateProof::SimplexWithinLevelPositiveRank {
            slopes: width,
            levels: levels.n_levels(),
            explicit_constant: columns.len() != width,
        },
    ));
    Ok(Some(result))
}
