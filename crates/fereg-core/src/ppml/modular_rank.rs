//! Exact dyadic rank lower bounds for PPML design certificates.
pub(super) const PRIME: u64 = (1 << 31) - 1;

/// Map the exact binary64 dyadic into the field. Two is invertible and
/// 2^31 = 1 mod PRIME, also for negative binary exponents.
pub(super) fn residue(value: f64) -> u64 {
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 2047) as i32;
    let fraction = bits & ((1 << 52) - 1);
    let mantissa = if exponent == 0 {
        fraction
    } else {
        fraction | (1 << 52)
    };
    let shift = (exponent.max(1) - 1023 - 52).rem_euclid(31);
    let magnitude = ((mantissa % PRIME) << shift) % PRIME;
    if value.is_sign_negative() && magnitude != 0 {
        PRIME - magnitude
    } else {
        magnitude
    }
}

fn inverse(mut x: u64) -> u64 {
    let mut power = PRIME - 2;
    let mut result = 1;
    while power != 0 {
        if power & 1 != 0 {
            result = result * x % PRIME;
        }
        x = x * x % PRIME;
        power >>= 1;
    }
    result
}

/// A nonzero diagonal marks an occupied, normalized pivot row. Products
/// stay below 2^62; no overflow or tolerance-based rank decision is used.
pub(super) fn insert(basis: &mut [u64], row: &mut [u64]) -> bool {
    let width = row.len();
    for (pivot, existing) in basis.chunks_exact_mut(width).enumerate() {
        let x = row[pivot];
        if x == 0 {
            continue;
        }
        if existing[pivot] == 0 {
            let reciprocal = inverse(x);
            for (dest, &value) in existing[pivot..].iter_mut().zip(&row[pivot..]) {
                *dest = value * reciprocal % PRIME;
            }
            return true;
        }
        for (entry, &known) in row[pivot..].iter_mut().zip(&existing[pivot..]) {
            *entry = (*entry + PRIME - x * known % PRIME) % PRIME;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangular_rows_allow_late_pivots_and_exact_dependencies() {
        let mut basis = [0; 25];
        assert!(insert(&mut basis, &mut [0, 0, 0, 0, 1]));
        assert!(insert(&mut basis, &mut [0, 2, 0, 1, 0]));
        assert!(!insert(&mut basis, &mut [0, 6, 0, 3, 2]));
        assert!(insert(&mut basis, &mut [0, 1, 0, 1, 0]));
    }

    #[test]
    fn dyadic_field_preserves_subnormals_signs_and_exact_cancellation() {
        assert_eq!(residue(-0.0), 0);
        assert_eq!(residue(0.5) * 2 % PRIME, 1);
        assert_eq!(residue(f64::from_bits(1)) * (1 << (1074 % 31)) % PRIME, 1);
        assert_eq!((residue(f64::MAX) + residue(-f64::MAX)) % PRIME, 0);
        let mut basis = [0; 4];
        assert!(insert(&mut basis, &mut [1, residue(0.5)]));
        assert!(!insert(&mut basis, &mut [2, 1]));
        assert!(insert(
            &mut basis,
            &mut [1, residue(f64::from_bits(0.5_f64.to_bits() + 1))]
        ));
    }
}
