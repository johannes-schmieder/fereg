//! Reference scales for the one-shot finite solve with an absorbed intercept.

use super::{
    NormalizedWeights, OneShotInputs, PreparedOlsFitView, ProjectedStorage, RawMomentCache,
};
use crate::error::Result;
use rayon::prelude::*;

pub(super) fn normalize<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    weights: NormalizedWeights<'_>,
    inputs: &mut OneShotInputs,
) -> Result<()> {
    if prepared.fit_has_absorbed_intercept() {
        let raw_moments = match &inputs.storage {
            ProjectedStorage::Dense(dense) => dense.raw.moments.as_ref(),
            ProjectedStorage::Centered(raw) => raw.moments.as_ref(),
            ProjectedStorage::Scratch(_) => None,
        };
        normalize_absorbed_reference_norms(
            prepared,
            weights,
            raw_moments,
            &mut inputs.reference_norms,
        )?;
    }
    Ok(())
}

/// The reference fold is ordered within a column. Independent columns can
/// borrow their raw sources and fold concurrently without a scan workspace.
pub(in crate::ols::prepared_fit) fn normalize_absorbed_reference_norms<
    P: PreparedOlsFitView + ?Sized,
>(
    prepared: &P,
    weights: NormalizedWeights<'_>,
    raw_moments: Option<&RawMomentCache>,
    norms: &mut [f64],
) -> Result<()> {
    let rows = prepared.fit_sample().n_active();
    let work = rows as u128 * norms.len() as u128;
    crate::parallel::stage_try_for_each(norms.par_iter_mut().enumerate(), work, |(column, norm)| {
        let summary = raw_moments
            .and_then(|moments| Some((*moments.scales.get(column + 1)?, moments.weight_sum)));
        let weight = |row| {
            weights
                .values
                .map_or(1.0, |values| values[row] / weights.raw_scale)
        };
        *norm = if let Some(values) =
            crate::ols::prepared_fit_view::prepared_continuous_range(prepared, column, 0..rows)
        {
            crate::ols::centered::reference_norm_with_summary(
                rows,
                *norm,
                summary,
                weights.values.is_none(),
                |row| Ok((values[row], weight(row))),
            )?
        } else {
            crate::ols::centered::reference_norm_with_summary(
                rows,
                *norm,
                summary,
                weights.values.is_none(),
                |row| Ok((prepared.fit_column_value(column, row)?, weight(row))),
            )?
        };
        Ok(())
    })
}
