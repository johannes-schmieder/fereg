use super::*;
use crate::fe::{
    certify_borrowed_intercept_candidate, BorrowedInterceptEffect, BorrowedMapWeights, LevelIndex,
    MapConfig,
};

/// One two-FE design: level 0 of the first effect starts with a zero-outcome
/// row (weight 1e-18, residual of order one) followed by positive rows whose
/// weight is one and whose residuals are about 1e-18.
fn floor_case() -> (Vec<u32>, Vec<u32>, Vec<f64>, Vec<f64>) {
    let rows = 12;
    let left = (0..rows).map(|row| u32::from(row >= 6)).collect::<Vec<_>>();
    let right = (0..rows).map(|row| (row % 3) as u32).collect::<Vec<_>>();
    let mut weights = vec![1.0; rows];
    let mut residual = vec![0.0; rows];
    for row in 0..rows {
        if row % 6 == 0 {
            weights[row] = 1e-18;
            residual[row] = 0.7 + row as f64 / 30.0;
        } else {
            residual[row] =
                1e-18 * (1.0 + (row as f64) / 7.0) * if row % 2 == 0 { 1.0 } else { -1.0 };
        }
    }
    (left, right, weights, residual)
}

#[test]
fn largest_weight_reference_removes_the_first_member_floor() {
    let (left, right, weights, residual) = floor_case();
    let li = LevelIndex::from_codes(&left, 2).unwrap();
    let ri = LevelIndex::from_codes(&right, 3).unwrap();
    let design = BorrowedInterceptFeDesign::new(
        left.len(),
        vec![
            BorrowedInterceptEffect::new(&left, 2, &li).unwrap(),
            BorrowedInterceptEffect::new(&right, 3, &ri).unwrap(),
        ],
    )
    .unwrap();
    let values = DenseMatrix::from_row_major(left.len(), 1, residual.clone()).unwrap();
    // Exact weighted level moments, in binary64 about zero: below 1e-17.
    let norm = residual
        .iter()
        .zip(&weights)
        .map(|(r, w)| w * r * r)
        .sum::<f64>()
        .sqrt();
    let config = MapConfig {
        tolerance: 1e-8,
        ..MapConfig::default()
    };
    let shared = certify_borrowed_intercept_candidate(
        &design,
        BorrowedMapWeights::Provided(&weights),
        &values,
        config,
    )
    .unwrap();
    let rechecked =
        largest_weight_certificate(&design, Some(&weights), 1.0, &values, &[norm], 1e-8).unwrap();
    // The shared certificate sees only its first-member rounding floor; the
    // re-check reproduces the exact moments' magnitude at the same scale.
    assert!(!shared.passed, "{shared:?}");
    assert!(rechecked.passed, "{rechecked:?}");
    assert!(shared.relative_infinity_norm > 100.0 * rechecked.relative_infinity_norm);
    // Witness: compensated level sums about zero (no shift) of these
    // well-scaled weighted products give the same largest relative moment.
    let mut witness = 0.0_f64;
    for (codes, levels) in [(&left, 2), (&right, 3)] {
        for level in 0..levels {
            let (mut sum, mut sum_c, mut total, mut total_c) = (0.0, 0.0, 0.0, 0.0);
            for row in (0..codes.len()).filter(|&row| codes[row] as usize == level) {
                neumaier_add(weights[row] * residual[row], &mut sum, &mut sum_c);
                neumaier_add(weights[row], &mut total, &mut total_c);
            }
            let total = total + total_c;
            witness = witness.max(((sum + sum_c) / total).abs() / norm * total.sqrt());
        }
    }
    assert!(
        (rechecked.relative_infinity_norm - witness).abs() <= 1e-12 * witness,
        "{rechecked:?} {witness}"
    );
    // A genuine moment fails both at the same tolerance.
    let mut shifted = residual;
    for value in shifted.iter_mut().take(6) {
        *value += 1e-6;
    }
    let values = DenseMatrix::from_row_major(left.len(), 1, shifted).unwrap();
    let failed =
        largest_weight_certificate(&design, Some(&weights), 1.0, &values, &[norm], 1e-8).unwrap();
    assert!(!failed.passed);
}
