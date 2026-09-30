//! Numeric dependence-unit identities; only the sealed retained axis is read.
use super::{filled, invalid};
use crate::{
    data::{NumericColumnView, Prepared},
    error::{ErrorCode, FeregError, Result},
};
#[derive(Debug, PartialEq)]
pub(super) struct Units {
    pub identities: Vec<f64>,
    pub codes: Vec<usize>,
    pub minimum_rows: usize,
    pub maximum_rows: usize,
}
impl Units {
    pub(super) fn new(
        sample: &Prepared,
        input: NumericColumnView<'_>,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Self> {
        if input.values.len() != sample.n_original()
            || input
                .missing
                .is_some_and(|mask| mask.len() != sample.n_original())
        {
            return Err(invalid(
                "biasunit() must cover the original observation axis",
            ));
        }
        let mut identities = filled(sample.n_active(), 0.)?;
        for (i, (value, &row)) in identities
            .iter_mut()
            .zip(sample.retained_rows())
            .enumerate()
        {
            if i % crate::threads::REDUCTION_CHUNK == 0 {
                interrupt()?;
            }
            if input.missing.is_some_and(|mask| mask[row]) || !input.values[row].is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    format!("biasunit() is missing on retained original row {row}"),
                ));
            }
            *value = if input.values[row] == 0. {
                0.
            } else {
                input.values[row]
            };
        }
        identities.sort_unstable_by(f64::total_cmp);
        interrupt()?;
        identities.dedup();
        if identities.len() < 2 {
            return Err(FeregError::new(
                ErrorCode::InsufficientObservations,
                "bias correction requires at least two independent units",
            ));
        }
        let mut codes = filled(sample.n_active(), 0)?;
        let mut counts = filled(identities.len(), 0usize)?;
        for (i, (code, &row)) in codes.iter_mut().zip(sample.retained_rows()).enumerate() {
            if i % crate::threads::REDUCTION_CHUNK == 0 {
                interrupt()?;
            }
            let value = if input.values[row] == 0. {
                0.
            } else {
                input.values[row]
            };
            *code = identities
                .binary_search_by(|v| v.total_cmp(&value))
                .map_err(|_| FeregError::new(ErrorCode::Internal, "bias unit identity absent"))?;
            counts[*code] += 1;
        }
        Ok(Self {
            identities,
            codes,
            minimum_rows: *counts.iter().min().expect("nonempty units"),
            maximum_rows: *counts.iter().max().expect("nonempty units"),
        })
    }
}
