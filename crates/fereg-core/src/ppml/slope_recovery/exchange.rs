//! A finite change of basis can exchange which regressor Stata omits. Try
//! the original convention only when a row-exact dependency involves the
//! transformed slope. An unrelated third omission never suffices.
use super::{numerically_omitted, PreparedPpmlIrlsFit, PreparedPpmlModelGeneration};
use crate::error::Result;
use crate::interrupt::row_checkpoint;
use crate::memory::{fallible_copy, fallible_filled};
use crate::ppml::{exact_dot, ExactNullspace};
use std::cmp::Ordering;

pub(super) struct Exchange {
    kept: Vec<usize>,
    omitted: usize,
}
impl Exchange {
    pub(super) fn candidate(
        g: &PreparedPpmlModelGeneration,
        fit: &PreparedPpmlIrlsFit,
        target: usize,
    ) -> Result<Option<Self>> {
        let slopes = fit.linear().basis().kept_generated_columns();
        let mut omitted = fit
            .coefficients()
            .iter()
            .filter(|c| numerically_omitted(c.status))
            .filter_map(|c| c.generated_column_index);
        let Some(column) = omitted.next() else {
            return Ok(None);
        };
        if omitted.next().is_some()
            || !slopes.contains(&target)
            || slopes.len() > 32
            || slopes
                .len()
                .saturating_add(1)
                .pow(2)
                .saturating_mul(g.original_rows().len())
                > 1 << 20
        {
            return Ok(None);
        }
        if !relation(g, slopes, column, target)? {
            return Ok(None);
        }
        Ok(Some(Self {
            kept: fallible_copy(slopes, "unable to reserve original alias columns")?,
            omitted: column,
        }))
    }
    pub(super) fn verified(
        &self,
        g: &PreparedPpmlModelGeneration,
        raw: &PreparedPpmlIrlsFit,
    ) -> Result<bool> {
        let kept = raw.linear().basis().kept_generated_columns();
        if self.kept.len() != kept.len() || !kept.contains(&self.omitted) {
            return Ok(false);
        }
        let mut lost = self.kept.iter().filter(|s| !kept.contains(s));
        let Some(&changed) = lost.next() else {
            return Ok(false);
        };
        if lost.next().is_some() {
            return Ok(false);
        }
        relation(g, kept, changed, self.omitted)
    }
}

fn relation(
    g: &PreparedPpmlModelGeneration,
    kept: &[usize],
    omitted: usize,
    participating: usize,
) -> Result<bool> {
    let Some(pos) = kept.iter().position(|&s| s == participating) else {
        return Ok(false);
    };
    let p = kept.len() + 1;
    let mut x = fallible_filled(p, 0., "unable to reserve original alias design row")?;
    let fill = |row, x: &mut [f64]| -> Result<()> {
        row_checkpoint(row)?;
        let value = |s| match &g.slope_recovery {
            Some(super::GenerationRecovery::Pair(pair)) if s == pair.target_column => {
                Ok(pair.original_target(row))
            }
            _ => g.fit_view().column_value(s, row),
        };
        for (v, &s) in x.iter_mut().zip(kept) {
            *v = value(s)?;
        }
        x[p - 1] = value(omitted)?;
        Ok(())
    };
    let mut space = ExactNullspace::new(p)?;
    for r in 0..g.original_rows().len() {
        fill(r, &mut x)?;
        space.insert(&x)?;
        if space.rank() == p {
            return Ok(false);
        }
    }
    if space.rank() != kept.len() || space.pivot(p - 1) {
        return Ok(false);
    }
    let mut free = fallible_filled(p, 0i64, "unable to reserve original alias direction")?;
    free[p - 1] = 1;
    let Some(c) = space.reconstruct(&free)? else {
        return Ok(false);
    };
    if c[pos] == 0 || c[p - 1] == 0 {
        return Ok(false);
    }
    for r in 0..g.original_rows().len() {
        fill(r, &mut x)?;
        if exact_dot(&x, &c) != Some(Ordering::Equal) {
            return Ok(false);
        }
    }
    Ok(true)
}
