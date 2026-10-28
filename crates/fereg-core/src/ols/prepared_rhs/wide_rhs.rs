//! Heap-backed numeric bindings for a design wider than the stack binding.
//!
//! Only the complete `[response, X]` batch uses them: the fill is an exact
//! copy of every source value, so a wide design builds the same matrix as the
//! generic per-cell recipe evaluator in bounded row tiles. The descriptor is
//! one pointer-sized pair per generated column and is charged by the memory
//! estimate; any irregularity or failed reservation keeps the generic builder.
use super::{fill_bound_rhs, MAX_BOUND_NUMERIC_COLUMNS};
use crate::data::NumericSourceBank;
use crate::error::Result;
use crate::ols::PreparedOlsColumnKind;

pub(crate) struct WideRhsColumns<'a> {
    columns: Vec<Option<&'a [f64]>>,
    rows: usize,
}

impl<'a> WideRhsColumns<'a> {
    /// Bind a numeric design wider than the stack binding, with the stack
    /// binding's row floor. Narrower designs keep the stack binding.
    pub(crate) fn bind(
        model: &crate::ols::PreparedOlsModel,
        sample: &'a crate::data::PreparedNumericSample<impl NumericSourceBank>,
    ) -> Option<Self> {
        let rows = sample.n_active();
        if rows < 4096 || model.columns.len() <= MAX_BOUND_NUMERIC_COLUMNS {
            return None;
        }
        let mut columns = Vec::new();
        columns.try_reserve_exact(model.columns.len()).ok()?;
        for column in &model.columns {
            columns.push(match column.kind {
                PreparedOlsColumnKind::Intercept => None,
                PreparedOlsColumnKind::Continuous {
                    prepared_source_index,
                    ..
                } => Some(
                    sample
                        .regressors
                        .get(prepared_source_index)
                        .filter(|values| values.len() == rows)?
                        .as_slice(),
                ),
                _ => return None,
            });
        }
        Some(Self { columns, rows })
    }

    pub(crate) fn fill_reserved_rhs(&self, response: &[f64], data: &mut Vec<f64>) -> Result<()> {
        fill_bound_rhs(&self.columns, self.rows, response, data)
    }
}
