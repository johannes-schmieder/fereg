use crate::error::{ErrorCode, FeregError, Result};

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
    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(values.len())
        .map_err(|_| allocation())?;
    for value in values {
        if !value.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "categorical numeric source contains a nonfinite value",
            ));
        }
        normalized.push(if *value == 0.0 { 0.0 } else { *value });
    }

    let mut levels = Vec::new();
    levels
        .try_reserve_exact(normalized.len())
        .map_err(|_| allocation())?;
    levels.extend_from_slice(&normalized);
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|left, right| left.total_cmp(right).is_eq());
    ensure_u32_levels(levels.len())?;
    let mut codes = Vec::new();
    codes
        .try_reserve_exact(normalized.len())
        .map_err(|_| allocation())?;
    for value in &normalized {
        codes.push(
            levels
                .binary_search_by(|candidate| candidate.total_cmp(value))
                .map(|position| position as u32)
                .expect("level was constructed from values"),
        );
    }
    Ok(EncodedFactor { codes, levels })
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
