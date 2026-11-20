//! A nonzero square minor of the stored finite positive-outcome rows proves
//! full rank over the rationals. No boundary rows or projection are needed.
//! Call only on a model with no absorbed channels; replay uses the same
//! immutable generation and raw column recipes.
use super::*;
use crate::ols::PreparedOlsColumnKind as Kind;

pub(in crate::ppml::prepared_separation) fn detect(
    model: &PreparedPpmlModelGeneration,
    positive: usize,
    limit: usize,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<DetectionProposal>> {
    let fit = model.fit_view();
    let columns = &fit.model().columns;
    // Include an implicit structural constant in the minor if present.
    // The report's finite width counts only the prepared coefficient slots.
    let implicit =
        fit.fit_structural_intercept() && !columns.iter().any(|c| c.kind == Kind::Intercept);
    let width = columns.len().saturating_add(usize::from(implicit));
    let square = width.saturating_mul(width);
    let mut work = limit
        .saturating_mul(8)
        .min(primal::ProofBudget::MAX_SPARSE_WORK);
    // The j-th accepted pivot costs at least j*width metered coordinates.
    // An inadmissible wide sparse design goes straight to its existing lanes.
    let minimum = (width as u128)
        .saturating_mul(width as u128)
        .saturating_mul(width.saturating_add(1) as u128)
        / 2;
    if columns.is_empty()
        || positive < width
        || square.saturating_add(width) > limit
        || minimum > work as u128
    {
        return Ok(None);
    }
    let read_cost = columns.iter().fold(usize::from(implicit), |sum, c| {
        sum.saturating_add(match &c.kind {
            Kind::CategoricalInteraction { components } => components.len().max(1),
            _ => 1,
        })
    });
    interrupt(PpmlInterruptPoint::SeparationSimplex)?;
    let mut basis = fallible_filled(
        square,
        0u64,
        "unable to reserve finite positive-rank pivots",
    )?;
    let mut row = fallible_filled(width, 0u64, "unable to reserve finite positive-rank row")?;
    let mut rank = 0;
    for (r, &y) in model.outcome().iter().enumerate() {
        crate::interrupt::row_checkpoint(r)?;
        let Some(left) = work.checked_sub(1) else {
            break;
        };
        work = left;
        if y <= 0. {
            continue;
        }
        let cost = read_cost.saturating_add(width.saturating_mul(rank + 1));
        let Some(left) = work.checked_sub(cost) else {
            break;
        };
        work = left;
        for (j, dest) in row[..columns.len()].iter_mut().enumerate() {
            crate::interrupt::row_checkpoint(j)?;
            *dest = residue(fit.column_value(j, r)?);
        }
        if implicit {
            row[width - 1] = 1;
        }
        rank += usize::from(insert(&mut basis, &mut row));
        if rank == width {
            if crate::diagnostics::solver_trace_enabled() {
                crate::diagnostics::emit_solver_trace(&format!(
                    "FEREG_SOLVER_TRACE|phase=finite_positive_rank|rows={}|columns={width}|inspected={}", model.outcome().len(), r+1
                ));
            }
            return Ok(Some(crate::ppml_separation::full_positive_rank_proposal(
                columns.len(),
            )));
        }
    }
    Ok(None)
}
