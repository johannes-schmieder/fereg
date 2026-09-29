//! Canonical observed-level encoding shared by finite columns, FEs and clusters.
mod interaction;
use crate::error::{copied_string, reserved, ErrorCode, FeregError, Result};
pub use interaction::{factorize_code_interaction, CategoricalCodeView, EncodedInteraction};
use std::{cmp::Ordering, collections::HashMap, hash::Hash};

#[derive(Debug, Clone, PartialEq)]
pub struct EncodedFactor<T> {
    pub codes: Vec<u32>,
    pub levels: Vec<T>,
}
impl<T> EncodedFactor<T> {
    #[must_use]
    pub fn n_levels(&self) -> usize {
        self.levels.len()
    }
}

pub fn factorize_f64(values: &[f64]) -> Result<EncodedFactor<f64>> {
    let encoded = encode(
        values.iter().map(|&value| {
            if !value.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "categorical numeric source contains a nonfinite value",
                ));
            }
            Ok(if value == 0.0 { 0 } else { value.to_bits() })
        }),
        |a, b| f64::from_bits(*a).total_cmp(&f64::from_bits(*b)),
    )?;
    let mut levels = reserved(encoded.levels.len())?;
    levels.extend(encoded.levels.into_iter().map(f64::from_bits));
    Ok(EncodedFactor {
        codes: encoded.codes,
        levels,
    })
}

pub fn factorize_i64(values: &[i64]) -> Result<EncodedFactor<i64>> {
    encode(values.iter().copied().map(Ok), Ord::cmp)
}

pub fn factorize_strings<S: AsRef<str>>(values: &[S]) -> Result<EncodedFactor<String>> {
    let encoded = encode(values.iter().map(|value| Ok(value.as_ref())), Ord::cmp)?;
    let mut levels = reserved(encoded.levels.len())?;
    for level in encoded.levels {
        levels.push(copied_string(level)?);
    }
    Ok(EncodedFactor {
        codes: encoded.codes,
        levels,
    })
}

// Hashing identifies equality only. Canonical sorting removes hash-seed and row-order
// dependence without allocating the Cartesian product of unobserved levels.
fn encode<T: Copy + Eq + Hash>(
    values: impl ExactSizeIterator<Item = Result<T>>,
    compare: impl Fn(&T, &T) -> Ordering,
) -> Result<EncodedFactor<T>> {
    let mut codes = reserved(values.len())?;
    let mut levels = Vec::new();
    let mut lookup = HashMap::new();
    for value in values {
        let value = value?;
        let code = if let Some(&code) = lookup.get(&value) {
            code
        } else {
            // Reserve u32::MAX as the representable level-count ceiling too.
            if levels.len() >= u32::MAX as usize {
                return Err(allocation());
            }
            let code = levels.len() as u32;
            lookup.try_reserve(1).map_err(|_| allocation())?;
            levels.try_reserve(1).map_err(|_| allocation())?;
            lookup.insert(value, code);
            levels.push(value);
            code
        };
        codes.push(code);
    }
    drop(lookup);
    let mut order = reserved(levels.len())?;
    order.extend(0..levels.len());
    order.sort_unstable_by(|&a, &b| compare(&levels[a], &levels[b]));
    let mut remap = reserved(levels.len())?;
    remap.resize(levels.len(), 0);
    let mut sorted = reserved(levels.len())?;
    for (code, old) in order.into_iter().enumerate() {
        remap[old] = code as u32;
        sorted.push(levels[old]);
    }
    for code in &mut codes {
        *code = remap[*code as usize];
    }
    Ok(EncodedFactor {
        codes,
        levels: sorted,
    })
}

fn allocation() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "unable to allocate categorical encoding",
    )
}
