//! With an intercept, the complement of emitted main-effect indicators
//! belongs to the raw model span. Whenever the varlist specifies only part
//! of a factor, all unrepresented levels form one residual cell on its axis.
use super::*;
use crate::ols::PreparedOlsColumnKind;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::ppml::prepared_separation) struct Factor {
    source: usize,
    emitted: Vec<bool>,
}

fn has_constant(model: &PreparedPpmlModelGeneration) -> bool {
    model.fit_view().has_absorbed_intercept()
        || model
            .model()
            .columns
            .iter()
            .any(|column| column.kind == PreparedOlsColumnKind::Intercept)
}

fn mark_emitted(generation: &PreparedPpmlModelGeneration, source: usize, present: &mut [bool]) {
    for column in &generation.model().columns {
        if let PreparedOlsColumnKind::Categorical {
            categorical_source_index,
            level_code: Some(level),
            ..
        } = column.kind
        {
            if categorical_source_index == source {
                present[level as usize] = true;
            }
        }
    }
}

/// Look for an observed, zero-only residual cell. The score is exactly
/// minus one plus the sum of distinct emitted indicators on this factor.
pub(super) fn find(
    generation: &PreparedPpmlModelGeneration,
    budget: ProofBudget,
) -> Result<Option<Factor>> {
    if !has_constant(generation) {
        return Ok(None);
    }
    let data = generation.fit_view();
    let mut work = budget.work_cells;
    for (source, axis) in data.categorical_sources().iter().enumerate() {
        let codes = axis.codes(data.fe_topology())?;
        let size = axis.n_levels(data.fe_topology())?;
        let charge = codes.len().saturating_add(generation.model().columns.len());
        if size.div_ceil(2) > budget.words || charge > work {
            break;
        }
        work -= charge;
        let mut emitted = fallible_filled(size, false, "unable to reserve exact factor cells")?;
        mark_emitted(generation, source, &mut emitted);
        if !emitted.contains(&true) || !emitted.contains(&false) {
            continue;
        }
        let mut residual = false;
        for (row, (&code, &y)) in codes.iter().zip(generation.outcome()).enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            if !emitted[code as usize] {
                if y > 0. {
                    residual = false;
                    break;
                }
                residual = true;
            }
        }
        if residual {
            return Ok(Some(Factor { source, emitted }));
        }
    }
    Ok(None)
}

impl Factor {
    pub(super) fn visit(
        &self,
        generation: &PreparedPpmlModelGeneration,
        mut inspect: impl FnMut(usize, Ordering, f64) -> bool,
    ) -> Result<bool> {
        if !has_constant(generation) {
            return Ok(false);
        }
        let data = generation.fit_view();
        let axis = &data.categorical_sources()[self.source];
        let codes = axis.codes(data.fe_topology())?;
        let size = axis.n_levels(data.fe_topology())?;
        let mut emitted = fallible_filled(size, false, "unable to replay exact factor witness")?;
        mark_emitted(generation, self.source, &mut emitted);
        if emitted != self.emitted {
            return Ok(false);
        }
        for (row, &code) in codes.iter().enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            let (sign, score) = if emitted[code as usize] {
                (Ordering::Equal, 0.)
            } else {
                (Ordering::Less, -1.)
            };
            if !inspect(row, sign, score) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
