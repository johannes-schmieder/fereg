//! Convert a certified constant dependency into identified model restrictions.
use super::PpmlWaldReference;
use crate::error::{ErrorCode, FeregError, Result};
use crate::ols::LinearConstantRepresentation;

pub(super) fn constant_wald_columns(
    kept: &[usize],
    constant: &LinearConstantRepresentation,
) -> Result<(Vec<usize>, Option<PpmlWaldReference>)> {
    if kept.len() != constant.loadings.len() || kept.is_empty() {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "constant Wald basis is misaligned",
        ));
    }
    let mut anchor = 0;
    for position in 1..kept.len() {
        if constant.loadings[position].abs() > constant.loadings[anchor].abs()
            || constant.loadings[position].abs() == constant.loadings[anchor].abs()
                && kept[position] < kept[anchor]
        {
            anchor = position;
        }
    }
    let mut columns = Vec::new();
    let mut loadings = Vec::new();
    columns
        .try_reserve_exact(kept.len() - 1)
        .map_err(|_| allocation())?;
    loadings
        .try_reserve_exact(kept.len() - 1)
        .map_err(|_| allocation())?;
    for (position, &slot) in kept.iter().enumerate() {
        if position != anchor {
            columns.push(slot);
            loadings.push(-constant.loadings[position] / constant.loadings[anchor]);
        }
    }
    Ok((
        columns,
        Some(PpmlWaldReference {
            slot: kept[anchor],
            loadings,
            span_error: constant.residual_relative_norm,
            span_tolerance: constant.relative_tolerance,
        }),
    ))
}

fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "unable to reserve PPML constant contrasts",
    )
}
