use crate::error::{ErrorCode, FeregError, Result};
use std::collections::HashMap;

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

pub fn factorize_i64(values: &[i64]) -> Result<EncodedFactor<i64>> {
    let mut levels = values.to_vec();
    levels.sort_unstable();
    levels.dedup();
    ensure_u32_levels(levels.len())?;
    let codes = values
        .iter()
        .map(|value| {
            levels
                .binary_search(value)
                .map(|position| position as u32)
                .expect("level was constructed from values")
        })
        .collect();
    Ok(EncodedFactor { codes, levels })
}

pub fn factorize_f64(values: &[f64]) -> Result<EncodedFactor<f64>> {
    let allocation = || {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve numeric categorical factorization storage",
        )
    };
    if let Some(encoded) = factorize_dense_integer_f64(values, allocation)? {
        return Ok(encoded);
    }
    // Factor variables normally contain far fewer levels than observations.
    // Building and sorting an observation-sized copy made that common case
    // O(N log N) and retained an N-sized capacity after deduplication. Assign
    // provisional codes in one hash pass, sort only the distinct levels, and
    // deterministically remap the codes to the canonical total order.
    let initial_levels = values.len().min(4_096);
    let mut provisional_by_bits = HashMap::<u64, u32>::new();
    provisional_by_bits
        .try_reserve(initial_levels)
        .map_err(|_| allocation())?;
    let mut provisional_levels = Vec::new();
    provisional_levels
        .try_reserve_exact(initial_levels)
        .map_err(|_| allocation())?;
    let mut codes = Vec::new();
    codes
        .try_reserve_exact(values.len())
        .map_err(|_| allocation())?;
    for value in values {
        if !value.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "categorical numeric source contains a nonfinite value",
            ));
        }
        let normalized = if *value == 0.0 { 0.0 } else { *value };
        let bits = normalized.to_bits();
        let code = if let Some(code) = provisional_by_bits.get(&bits) {
            *code
        } else {
            let next_level = provisional_levels.len();
            ensure_u32_levels(next_level.saturating_add(1))?;
            provisional_by_bits
                .try_reserve(1)
                .map_err(|_| allocation())?;
            provisional_levels
                .try_reserve(1)
                .map_err(|_| allocation())?;
            let code = next_level as u32;
            provisional_by_bits.insert(bits, code);
            provisional_levels.push(normalized);
            code
        };
        codes.push(code);
    }
    drop(provisional_by_bits);

    let mut order = Vec::new();
    order
        .try_reserve_exact(provisional_levels.len())
        .map_err(|_| allocation())?;
    order.extend(0..provisional_levels.len());
    order.sort_unstable_by(|left, right| {
        provisional_levels[*left].total_cmp(&provisional_levels[*right])
    });
    let mut remap = Vec::new();
    remap
        .try_reserve_exact(provisional_levels.len())
        .map_err(|_| allocation())?;
    remap.resize(provisional_levels.len(), 0_u32);
    let mut levels = Vec::new();
    levels
        .try_reserve_exact(provisional_levels.len())
        .map_err(|_| allocation())?;
    for (canonical, provisional) in order.into_iter().enumerate() {
        remap[provisional] = canonical as u32;
        levels.push(provisional_levels[provisional]);
    }
    for code in &mut codes {
        *code = remap[*code as usize];
    }
    Ok(EncodedFactor { codes, levels })
}

fn factorize_dense_integer_f64<F>(
    values: &[f64],
    allocation: F,
) -> Result<Option<EncodedFactor<f64>>>
where
    F: Fn() -> FeregError + Copy,
{
    if values.is_empty() {
        return Ok(Some(EncodedFactor {
            codes: Vec::new(),
            levels: Vec::new(),
        }));
    }
    let mut minimum = i64::MAX;
    let mut maximum = i64::MIN;
    for value in values {
        if !value.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "categorical numeric source contains a nonfinite value",
            ));
        }
        let normalized = if *value == 0.0 { 0.0 } else { *value };
        if normalized.fract() != 0.0
            || normalized < i64::MIN as f64
            || normalized >= i64::MAX as f64
        {
            return Ok(None);
        }
        let integer = normalized as i64;
        minimum = minimum.min(integer);
        maximum = maximum.max(integer);
    }
    let range_i128 = i128::from(maximum) - i128::from(minimum) + 1;
    let dense_limit = values.len().saturating_mul(2).max(4_096);
    let Ok(range) = usize::try_from(range_i128) else {
        return Ok(None);
    };
    if range > dense_limit {
        return Ok(None);
    }

    let mut canonical_by_offset = Vec::new();
    canonical_by_offset
        .try_reserve_exact(range)
        .map_err(|_| allocation())?;
    canonical_by_offset.resize(range, u32::MAX);
    for value in values {
        let integer = if *value == 0.0 { 0 } else { *value as i64 };
        let offset =
            usize::try_from(i128::from(integer) - i128::from(minimum)).map_err(|_| allocation())?;
        canonical_by_offset[offset] = 0;
    }

    let n_levels = canonical_by_offset
        .iter()
        .filter(|code| **code != u32::MAX)
        .count();
    ensure_u32_levels(n_levels)?;
    let mut levels = Vec::new();
    levels
        .try_reserve_exact(n_levels)
        .map_err(|_| allocation())?;
    let mut next = 0_u32;
    for (offset, code) in canonical_by_offset.iter_mut().enumerate() {
        if *code == u32::MAX {
            continue;
        }
        *code = next;
        let integer = i128::from(minimum) + offset as i128;
        levels.push(integer as f64);
        next = next.checked_add(1).ok_or_else(allocation)?;
    }

    let mut codes = Vec::new();
    codes
        .try_reserve_exact(values.len())
        .map_err(|_| allocation())?;
    for value in values {
        let integer = if *value == 0.0 { 0 } else { *value as i64 };
        let offset =
            usize::try_from(i128::from(integer) - i128::from(minimum)).map_err(|_| allocation())?;
        codes.push(canonical_by_offset[offset]);
    }
    Ok(Some(EncodedFactor { codes, levels }))
}

pub fn factorize_strings<S: AsRef<str>>(values: &[S]) -> Result<EncodedFactor<String>> {
    let mut levels: Vec<String> = values
        .iter()
        .map(|value| value.as_ref().to_owned())
        .collect();
    levels.sort();
    levels.dedup();
    ensure_u32_levels(levels.len())?;
    let codes = values
        .iter()
        .map(|value| {
            levels
                .binary_search_by(|candidate| candidate.as_str().cmp(value.as_ref()))
                .map(|position| position as u32)
                .expect("level was constructed from values")
        })
        .collect();
    Ok(EncodedFactor { codes, levels })
}

pub fn interaction_codes(sources: &[&[u32]]) -> Result<EncodedFactor<Vec<u32>>> {
    if sources.is_empty() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "an interaction requires at least one source",
        ));
    }
    let len = sources[0].len();
    if sources.iter().any(|source| source.len() != len) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "interaction sources have different observation counts",
        ));
    }
    let tuples: Vec<Vec<u32>> = (0..len)
        .map(|row| sources.iter().map(|source| source[row]).collect())
        .collect();
    let mut levels = tuples.clone();
    levels.sort_unstable();
    levels.dedup();
    ensure_u32_levels(levels.len())?;
    let codes = tuples
        .iter()
        .map(|tuple| {
            levels
                .binary_search(tuple)
                .map(|position| position as u32)
                .expect("level was constructed from tuples")
        })
        .collect();
    Ok(EncodedFactor { codes, levels })
}

fn ensure_u32_levels(levels: usize) -> Result<()> {
    if levels > u32::MAX as usize {
        Err(FeregError::new(
            ErrorCode::InvalidData,
            "factor contains more than u32::MAX levels",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_codes_are_row_order_invariant() {
        let encoded = factorize_i64(&[20, 10, 20, -1]).expect("factorize");
        assert_eq!(encoded.levels, vec![-1, 10, 20]);
        assert_eq!(encoded.codes, vec![2, 1, 2, 0]);
    }

    #[test]
    fn floating_zero_is_canonicalized() {
        let encoded = factorize_f64(&[-0.0, 0.0, 2.5]).expect("factorize");
        assert_eq!(encoded.levels, vec![0.0, 2.5]);
        assert_eq!(encoded.codes, vec![0, 0, 1]);
    }

    #[test]
    fn dense_integer_codes_are_canonical_with_gaps_and_negatives() {
        let encoded = factorize_f64(&[2.0, -1.0, 2.0, 0.0, -1.0]).expect("factorize");
        assert_eq!(encoded.levels, vec![-1.0, 0.0, 2.0]);
        assert_eq!(encoded.codes, vec![2, 0, 2, 1, 0]);
    }

    #[test]
    fn string_codes_are_collision_safe() {
        let encoded = factorize_strings(&["a#b", "a", "a#b", "b"]).expect("factorize");
        assert_eq!(encoded.levels, vec!["a", "a#b", "b"]);
        assert_eq!(encoded.codes, vec![1, 0, 1, 2]);
    }

    #[test]
    fn tuple_interactions_do_not_concatenate_strings() {
        let first = [1, 1, 12];
        let second = [23, 2, 3];
        let encoded = interaction_codes(&[&first, &second]).expect("interaction");
        assert_eq!(encoded.n_levels(), 3);
        assert_ne!(encoded.codes[0], encoded.codes[2]);
    }
}
