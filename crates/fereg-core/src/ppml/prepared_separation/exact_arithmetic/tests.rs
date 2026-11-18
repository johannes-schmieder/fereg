use super::*;
use std::cmp::Ordering::{Equal, Greater, Less};

#[test]
fn rational_reconstruction_preserves_strict_signs_and_clears_denominators() {
    let positives = [[3., 0., 1.], [0., 5., 1.]];
    let mut basis = ExactNullspace::new(3).unwrap();
    for row in &positives {
        basis.insert(row).unwrap();
    }
    assert_eq!(basis.rank(), 2);
    assert!(basis.pivot(0) && basis.pivot(1) && !basis.pivot(2));
    let b = basis.reconstruct(&[0, 0, 9]).unwrap().unwrap();
    assert_eq!(b, [-5, -3, 15]);
    assert!(positives
        .iter()
        .all(|row| exact_dot(row, &b) == Some(Equal)));
    assert_eq!(exact_dot(&[1., 1., 0.], &b), Some(Less));
    assert_eq!(exact_dot(&[-1., -1., 0.], &b), Some(Greater));
    assert_eq!(basis.reconstruct(&[1, 1, 0]).unwrap(), None);
}

#[test]
fn mixed_proposal_only_has_one_strict_negative_raw_row() {
    let eps = 2f64.powi(-26);
    for units in [0.25, 1., 32.] {
        let positives = [
            [1., 0., 0., 0.],
            [1., 1., units, 0.],
            [1., 2., (2. + eps) * units, 0.],
        ];
        let mut basis = ExactNullspace::new(4).unwrap();
        for row in &positives {
            basis.insert(row).unwrap();
        }
        let b = basis.reconstruct(&[0, 0, 0, -1]).unwrap().unwrap();
        assert!(positives
            .iter()
            .all(|row| exact_dot(row, &b) == Some(Equal)));
        assert_eq!(exact_dot(&[1., 0., units, 0.], &b), Some(Equal));
        assert_eq!(exact_dot(&[1., 0., 0., 1.], &b), Some(Less));
    }
    assert_eq!(
        exact_dot(&[1., 2., 2. + eps, 0.], &[0, 1, -1, -1]),
        Some(Less)
    );
}

#[test]
fn a_bad_prime_can_propose_a_false_equality_but_cannot_verify_it() {
    let row = [1., PRIME as f64];
    let mut basis = ExactNullspace::new(2).unwrap();
    basis.insert(&[1., 0.]).unwrap();
    basis.insert(&row).unwrap();
    assert_eq!(basis.rank(), 1);
    let b = basis.reconstruct(&[0, -1]).unwrap().unwrap();
    assert_eq!(exact_dot(&row, &b), Some(Less));
}

#[test]
fn exact_score_resolves_extreme_exponents_subnormals_carries_and_signs() {
    let tiny = f64::from_bits(1);
    let row = [f64::MAX, tiny, f64::MAX, -0.];
    for sign in [-1i64, 0, 1] {
        let b = [i64::MAX, sign, -i64::MAX, i64::MIN];
        assert_eq!(exact_dot(&row, &b), Some(sign.cmp(&0)));
    }
    let large = [f64::MAX; 37];
    let mut b = [i64::MAX; 37];
    for value in &mut b[..18] {
        *value = -i64::MAX;
    }
    b[36] = 0;
    assert_eq!(exact_dot(&large, &b), Some(Equal));
    b[36] = i64::MIN;
    assert_eq!(exact_dot(&large, &b), Some(Less));
    assert_eq!(exact_dot(&[f64::INFINITY], &[0]), None);
    assert_eq!(exact_dot(&[1.], &[]), None);
}

#[test]
fn score_rounds_only_once_after_cancellation_and_resolves_halfway_cases() {
    let tiny = f64::from_bits(1);
    let half = 2f64.powi(-53);
    let next = f64::from_bits(1f64.to_bits() + 1);
    assert_eq!(exact_score(&[1., half], &[1, 1]), Some((Greater, 1.)));
    assert_eq!(
        exact_score(&[1., half, tiny], &[1, 1, 1]),
        Some((Greater, next))
    );
    assert_eq!(
        exact_score(&[next, half], &[1, 1]),
        Some((Greater, f64::from_bits(1f64.to_bits() + 2)))
    );
    assert_eq!(
        exact_score(&[f64::MAX, tiny, f64::MAX], &[i64::MAX, -1, -i64::MAX]),
        Some((Less, -tiny))
    );
    let overflow_half = 2f64.powi(970);
    assert_eq!(
        exact_score(&[f64::MAX, overflow_half, tiny], &[1, 1, -1]),
        Some((Greater, f64::MAX))
    );
    assert_eq!(
        exact_score(&[f64::MAX, overflow_half], &[1, 1]),
        Some((Greater, f64::INFINITY))
    );
}
