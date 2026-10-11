//! Independent integer-arithmetic PPML score moments and scale witness.
//! Included directly by core tests to avoid a core/testkit dependency cycle.

pub fn moments(y: &[i64], mu: &[i64], q: &[i64], x: &[i64], scale: f64) -> (f64, f64) {
    let mut score = 0_i128;
    let mut square = 0_i128;
    for row in 0..y.len() {
        score += i128::from(q[row]) * i128::from(y[row] - mu[row]) * i128::from(x[row]);
        square += i128::from(q[row]) * i128::from(mu[row]) * i128::from(x[row]).pow(2);
    }
    assert!(score.abs() <= 1_i128 << 53 && square <= 1_i128 << 53);
    (score as f64 * scale, square as f64 * scale)
}

pub fn score_scale(y: &[i64], mu: &[i64], q: &[i64], scale: f64) -> f64 {
    let mut residual = 0_i128;
    let mut reference = 0_i128;
    for row in 0..y.len() {
        assert!(matches!(mu[row], 1 | 2));
        let factor = i128::from(q[row]) * i128::from(2 / mu[row]);
        residual += factor * i128::from(y[row] - mu[row]).pow(2);
        reference += factor * i128::from(y[row]).pow(2);
    }
    let square = residual.max(reference);
    assert!(square <= 1_i128 << 53);
    (square as f64 * (scale / 2.)).sqrt()
}
