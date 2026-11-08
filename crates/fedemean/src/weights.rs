//! Observation weights and their validation.

use crate::error::{Error, Result};
use rayon::prelude::*;

/// Observation weights of the projection metric `sum_i w_i r_i^2`. Provided
/// weights must be finite and positive; zero-weight rows are the caller's
/// sample decision. Internally weights are divided by their maximum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Weights<'a> {
    Unit,
    Values(&'a [f64]),
}

/// Validated weights and their maximum (`raw_scale`; 1 for unit weights).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Validated<'a> {
    pub(crate) values: Option<&'a [f64]>,
    pub(crate) raw_scale: f64,
}

/// One parallel pass: validity, maximum and minimum. Division by the positive
/// maximum is monotone, so some normalized weight underflows to zero exactly
/// when the minimum's does.
pub(crate) fn validate(weights: Weights<'_>, nrows: usize) -> Result<Validated<'_>> {
    match weights {
        Weights::Unit => Ok(Validated {
            values: None,
            raw_scale: 1.0,
        }),
        Weights::Values(values) => {
            if values.len() != nrows {
                return Err(Error::InvalidInput(
                    "borrowed MAP weight and observation counts differ",
                ));
            }
            const CHUNK: usize = 1 << 16;
            let (valid, raw_scale, minimum) = values
                .par_chunks(CHUNK)
                .map(|chunk| {
                    chunk.iter().fold(
                        (true, 0.0_f64, f64::INFINITY),
                        |(valid, maximum, minimum), &weight| {
                            (
                                valid && weight.is_finite() && weight > 0.0,
                                maximum.max(weight),
                                minimum.min(weight),
                            )
                        },
                    )
                })
                .reduce(
                    || (true, 0.0_f64, f64::INFINITY),
                    |a, b| (a.0 && b.0, a.1.max(b.1), a.2.min(b.2)),
                );
            if !valid {
                return Err(Error::InvalidData(
                    "borrowed MAP weights must be finite and positive",
                ));
            }
            if minimum / raw_scale == 0.0 {
                return Err(Error::InvalidData(
                    "borrowed MAP normalized weight underflowed to zero",
                ));
            }
            Ok(Validated {
                values: Some(values),
                raw_scale,
            })
        }
    }
}
