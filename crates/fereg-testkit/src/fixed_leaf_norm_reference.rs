//! Independent analytic norm witness for exact integer/power-of-two inputs.
//! Included directly by core unit tests to avoid a core/testkit dependency cycle.

pub fn integer_power_norm(terms: impl Iterator<Item = i64>, scale: f64) -> f64 {
    let square: u128 = terms
        .map(|term| {
            let magnitude = u128::from(term.unsigned_abs());
            magnitude * magnitude
        })
        .sum();
    // These fixtures keep the exact integer sum within binary64's integer range.
    assert!(square <= (1_u128 << 53));
    (square as f64).sqrt() * scale
}

/// Frozen pre-E1 observation-order norm state for one-leaf exact-scope checks.
/// Numerical accuracy is separately witnessed by `integer_power_norm` above.
pub fn legacy_scalar_norm_state(values: impl Iterator<Item = f64>) -> (f64, f64, f64) {
    let mut scale = 0.0_f64;
    let mut sum = 1.0_f64;
    for value in values {
        let absolute = value.abs();
        if absolute == 0.0 {
            continue;
        }
        if scale < absolute {
            let ratio = scale / absolute;
            sum = 1.0 + sum * ratio * ratio;
            scale = absolute;
        } else {
            let ratio = absolute / scale;
            sum += ratio * ratio;
        }
    }
    let norm = if scale == 0.0 {
        0.0
    } else {
        scale * sum.sqrt()
    };
    (norm, scale, sum)
}
