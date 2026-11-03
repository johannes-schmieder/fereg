use super::*;

#[test]
fn refined_stopping_resolves_small_row_reconstruction() {
    // Independent replay of a weighted sign-cube clustered covariance. The
    // huge normalized-intercept variance must not mask the z/constant entry.
    let matrix = [
        0.12499999999999367,
        1.868540048376557e-17,
        -68828320.80200152,
        1.868540048376557e-17,
        0.12499999999999367,
        -1.028867791048445e-8,
        -68828320.80200152,
        -1.028867791048445e-8,
        3.78987019553878e16,
    ];
    let mut reservation = SystemPreparedInferenceReservation;
    let ordinary = symmetric_eigensystem(&matrix, 3, false, &mut reservation).unwrap();
    let refined = symmetric_eigensystem(&matrix, 3, true, &mut reservation).unwrap();
    assert!(ordinary.reconstruction_error > 5e-8);
    assert!(refined.reconstruction_error < 1e-12);
    assert!(refined.orthogonality_error < 1e-12);
    assert_eq!(ordinary.values, refined.values);
}

#[test]
fn refined_clipping_preserves_reported_euclidean_projection() {
    // The first coordinate is a negative eigenspace. The lower 2x2 block is
    // positive definite (determinant 1e16 - 1e-8), so the exact Euclidean PSD
    // projection changes only entry (0,0) to zero. In particular, the 1e-4
    // coupling remains in the original reported coordinates after clipping.
    let matrix = [-1e-8, 0.0, 0.0, 0.0, 1.0, 1e-4, 0.0, 1e-4, 1e16];
    let mut reservation = SystemPreparedInferenceReservation;
    let ordinary = symmetric_eigensystem(&matrix, 3, false, &mut reservation).unwrap();
    // The ordinary policy retains its exact pre-change stopping signature.
    assert_eq!(ordinary.values, [-1e-8, 1.0, 1e16]);
    assert_eq!(
        ordinary.vectors,
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
    );
    let refined = symmetric_eigensystem(&matrix, 3, true, &mut reservation).unwrap();
    assert!(refined.reconstruction_error < 1e-12);
    assert!(refined.orthogonality_error < 1e-12);
    let (clipped, count) = reconstruct_clipped(&refined, 3, &mut reservation).unwrap();
    assert_eq!(count, 1);
    let mut expected = matrix;
    expected[0] = 0.0;
    for (position, (actual, expected)) in clipped.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() <= 1e-14 * expected.abs().max(1.0),
            "projection entry {position}: actual={actual:.17e}, expected={expected:.17e}"
        );
    }
    assert!(certify_clipped_reconstruction(&refined, &clipped, 3).unwrap() < 1e-12);
}

#[test]
fn stopping_scale_skips_zero_rows_and_never_raises_tiny_scales() {
    assert_eq!(stopping_scale(&[], 0).unwrap(), 0.0);
    assert_eq!(stopping_scale(&[0.0; 4], 2).unwrap(), 0.0);
    assert_eq!(stopping_scale(&[0.0, 0.0, 0.0, 2.0], 2).unwrap(), 2.0);
    let tiny = f64::from_bits(1);
    let scale = stopping_scale(&[tiny, 0.0, 0.0, 1.0], 2).unwrap();
    assert_eq!(scale, tiny);
    assert_eq!(128.0 * f64::EPSILON * scale, 0.0);
    for invalid in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert!(stopping_scale(&[invalid, 0.0, 0.0, 1.0], 2).is_err());
    }
    assert!(stopping_scale(&[1.0], 2).is_err());
}
