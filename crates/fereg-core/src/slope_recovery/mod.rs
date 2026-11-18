//! Recode a weak pair within the estimator's finalized design. The saved
//! raw column lets inference certify (or restore) the exact public coordinates.
pub mod absorbed;
mod basis;
pub mod mapping;
mod screen;

use crate::error::{invalid_argument, Result};
use crate::interrupt::row_checkpoint;
use crate::ols::{PreparedOlsColumnKind, PreparedOlsData};
use mapping::{PairMap, PredictionCheck};

/// A single exactly reversible change on a finalized compact row axis.
#[derive(Debug, PartialEq)]
pub struct Recovery {
    pub source_column: usize,
    pub target_column: usize,
    pub map: PairMap,
    storage_index: usize,
    original: Vec<f64>,
}

impl Recovery {
    pub fn owned_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.original)
    }

    pub fn original_target(&self, row: usize) -> f64 {
        self.original[row]
    }

    /// Replay the actual published coefficient order, including terms between
    /// the two mapped columns. In binary64, even equal pair subtotals do not
    /// ensure that the complete finite predictor is accurately representable.
    pub fn check_full_predictions(
        &self,
        prepared: &PreparedOlsData,
        check: &PredictionCheck,
        coefficients: &[f64],
    ) -> Result<()> {
        self.visit_full_predictions(prepared, check, coefficients, |_, _| Ok(()))
    }

    /// Also expose each original ordered finite predictor to the PPML link
    /// and saved-effect checks, without streaming the compact design twice.
    pub fn visit_full_predictions(
        &self,
        prepared: &PreparedOlsData,
        check: &PredictionCheck,
        coefficients: &[f64],
        mut visit: impl FnMut(usize, f64) -> Result<()>,
    ) -> Result<()> {
        if prepared.sample().n_active() != self.original.len()
            || prepared.report_plan().public_coefficient_count() != coefficients.len()
        {
            return Err(invalid_argument(
                "slope recovery belongs to a different fit",
            ));
        }
        for (row, &original) in self.original.iter().enumerate() {
            row_checkpoint(row)?;
            check.check_row(
                prepared.column_value(self.source_column, row)?,
                original,
                prepared.column_value(self.target_column, row)?,
            )?;
            let (mut raw, mut stable) = (0.0, 0.0);
            for (col, descriptor) in prepared.model().columns.iter().enumerate() {
                let slot = descriptor.coefficient_slot.get() as usize - 1;
                let value = prepared.column_value(col, row)?;
                let raw_value = if col == self.target_column {
                    original
                } else {
                    value
                };
                raw += raw_value * check.posted_coefficient(slot, coefficients[slot]);
                stable += value * check.stable_coefficient(slot, coefficients[slot]);
            }
            mapping::check_predictor_values(raw, stable)?;
            visit(row, raw)?;
        }
        Ok(())
    }

    pub fn restore_design(mut self, prepared: &mut PreparedOlsData) -> Result<()> {
        prepared
            .sample
            .swap_recovery_column(self.storage_index, &mut self.original)
    }
}

// Limit screening work independently of factor expansion. A search limit
// preserves the usual certified estimator and cannot qualify a missing fit.
const SEARCH_WIDTH: usize = 32;
const MAX_PAIR_ROWS: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Default)]
struct Column {
    generated: usize,
    storage: usize,
    slot: usize,
    sketch: screen::Sketch,
    mutable: bool,
}

impl Column {
    fn identify(prepared: &PreparedOlsData, generated: usize) -> Option<Self> {
        let col = &prepared.model().columns[generated];
        let PreparedOlsColumnKind::Continuous {
            source,
            prepared_source_index,
        } = col.kind
        else {
            return None;
        };
        // The alternative column must not change any other RHS or absorbed
        // heterogeneous slope. FE and cluster partitions already own their codes.
        let unique = !prepared.model().columns.iter().enumerate().any(|(other, col)| {
            other != generated && matches!(col.kind, PreparedOlsColumnKind::Continuous {source: other_source, ..} if other_source == source)
        });
        let shared = prepared.effect_specs.iter().any(|effect| {
            effect.slopes.iter().any(|s| s.source == source) || effect.sources.contains(&source)
        }) || prepared
            .categorical_sources()
            .iter()
            .any(|s| s.source() == source);
        Some(Self {
            generated,
            storage: prepared_source_index,
            slot: prepared
                .report_plan()
                .original_slot(col.coefficient_slot)
                .ok()?
                .get() as usize
                - 1,
            mutable: unique && !shared,
            ..Self::default()
        })
    }
}

// One unordered pair owns one budget charge. Both permissible orientations
// are certified using constant scratch. Only the winner gets a row vector.
fn select_orientation(
    columns: [Column; 2],
    values: [&[f64]; 2],
    witnesses: &[Column],
    contains_constant: bool,
    budget: &mut usize,
) -> Result<Option<(usize, basis::PairwiseDifference)>> {
    let screens = [0, 1].map(|source| {
        let target = 1 - source;
        if !columns[target].mutable {
            return None;
        }
        let mut s = screen::PairScreen::new(
            columns[source].sketch,
            columns[target].sketch,
            contains_constant,
        )?;
        witnesses
            .iter()
            .flat_map(|c| c.sketch.extrema_rows.into_iter().flatten())
            .all(|row| s.accepts_row(values[source][row], values[target][row]))
            .then_some(s)
    });
    if screens.iter().all(Option::is_none) {
        return Ok(None);
    }
    *budget -= values[0].len(); // the caller admitted this pair's size
    let mut best: Option<(usize, basis::PairwiseDifference)> = None;
    for (source, screen) in screens.into_iter().enumerate() {
        let Some(mut screen) = screen else {
            continue;
        };
        let target = 1 - source;
        let (x, z) = (values[source], values[target]);
        if !screen.accepts_positions(x, z) {
            continue;
        }
        let bounds = |c: Column| [c.sketch.minimum, c.sketch.maximum];
        let centered =
            contains_constant.then_some((bounds(columns[source]), bounds(columns[target])));
        let Some(candidate) = basis::discover(x, z, centered)? else {
            continue;
        };
        // A complete numerical tie keeps the earlier requested source. Both
        // changes were certified, and publication is checked in either order.
        if best.is_none_or(|(_, current)| candidate.priority() < current.priority()) {
            best = Some((source, candidate));
        }
    }
    Ok(best)
}

/// Search explicit continuous columns for a well-resolved independent
/// direction that is weak in the raw scale. Only return a map after checking
/// the complete data. The ceiling includes existing preparation ownership.
pub fn prepare(prepared: &mut PreparedOlsData, ceiling: u128) -> Result<Option<Recovery>> {
    let mut columns = [Column::default(); SEARCH_WIDTH];
    let mut count = 0;
    for (index, col) in prepared.model().columns.iter().enumerate() {
        if !matches!(col.kind, PreparedOlsColumnKind::Continuous { .. }) {
            continue;
        }
        if count == SEARCH_WIDTH {
            return Ok(None);
        }
        if let Some(column) = Column::identify(prepared, index) {
            columns[count] = column;
            count += 1;
        }
    }
    let rows = prepared.sample().n_active();
    if count < 2 || rows < 2 {
        return Ok(None);
    }
    let candidates = &mut columns[..count];
    for column in candidates.iter_mut() {
        column.sketch = screen::Sketch::new(
            prepared
                .sample()
                .regressors
                .get(column.storage)
                .ok_or_else(|| invalid_argument("missing slope source during recovery"))?,
        )?;
    }
    // Count candidate rows after permutation-invariant exclusions only.
    // The exact basis check uses a fixed number of passes per admitted pair.
    let mut remaining = MAX_PAIR_ROWS.max(rows);
    let contains_constant = prepared.structural_intercept() || prepared.has_absorbed_intercept();
    for (index, &left) in candidates.iter().enumerate() {
        for &right in &candidates[index + 1..] {
            if remaining < rows {
                return Ok(None);
            }
            if left.storage == right.storage || (!left.mutable && !right.mutable) {
                continue;
            }
            let bank = &prepared.sample().regressors;
            let get = |c: Column| {
                bank.get(c.storage)
                    .map(Vec::as_slice)
                    .ok_or_else(|| invalid_argument("missing slope source during recovery"))
            };
            let values = [get(left)?, get(right)?];
            let columns = [left, right];
            let Some((side, basis)) = select_orientation(
                columns,
                values,
                candidates,
                contains_constant,
                &mut remaining,
            )?
            else {
                continue;
            };
            let (source, target) = (columns[side], columns[1 - side]);
            let required = (rows as u128) * 8 + prepared.owned_buffer_bytes();
            let mut original =
                basis.materialize(values[side], values[1 - side], required <= ceiling)?;
            let map = PairMap {
                source: source.slot,
                target: target.slot,
                multiple: basis.coefficient,
                scale: basis.scale,
            };
            prepared
                .sample
                .swap_recovery_column(target.storage, &mut original)?;
            return Ok(Some(Recovery {
                source_column: source.generated,
                target_column: target.generated,
                storage_index: target.storage,
                map,
                original,
            }));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_source_marginals_keep_the_first_requested_source() {
        let step = 2_f64.powi(-18);
        let x = [0., 1., 1. + step, 2.];
        let z = [0., 1. + step, 1., 2.];
        for values in [[x.as_slice(), z.as_slice()], [z.as_slice(), x.as_slice()]] {
            let columns = values.map(|v| Column {
                sketch: screen::Sketch::new(v).unwrap(),
                mutable: true,
                ..Column::default()
            });
            let mut budget = x.len();
            let (source, change) = select_orientation(columns, values, &columns, true, &mut budget)
                .unwrap()
                .unwrap();
            assert_eq!(source, 0);
            assert_eq!(change.coefficient, 1.);
        }
    }

    #[test]
    fn both_orders_certify_the_short_shear_with_one_pair_charge() {
        let a = [-2., -1., 0., 1., 2., 1.];
        let w = [-0.5, 0.75, -1., 2., -2., 0.25];
        let x = a.map(|v| 32. * v);
        let z = std::array::from_fn::<_, 6, _>(|i| (-3. * a[i] + 2_f64.powi(-18) * w[i]) / 16.);
        for values in [[x.as_slice(), z.as_slice()], [z.as_slice(), x.as_slice()]] {
            let columns = values.map(|v| Column {
                sketch: screen::Sketch::new(v).unwrap(),
                mutable: true,
                ..Column::default()
            });
            let mut budget = x.len();
            let (index, change) = select_orientation(columns, values, &columns, true, &mut budget)
                .unwrap()
                .unwrap();
            assert_eq!(values[index], x);
            assert_eq!(change.coefficient, -3. / 512.);
            assert_eq!(budget, 0);
        }
    }
}
