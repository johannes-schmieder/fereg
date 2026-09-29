//! Stable Poisson scalar formulas harvested from legacy ppml/family.rs.
use super::numerical;
use crate::Result;
pub(super) fn half_deviance(y: f64, mu: f64, log_mean: f64, log_y: Option<f64>) -> Result<f64> {
    if y == 0.0 {
        return Ok(mu);
    }
    if y == mu {
        return Ok(0.0);
    }
    let difference = y - mu;
    let relative_difference = difference / mu;
    let value = if relative_difference.is_finite() && relative_difference.abs() <= 0.5 {
        mu * one_plus_x_log1p_minus_x(relative_difference)
    } else {
        y * (log_y.unwrap_or_else(|| y.ln()) - log_mean) - difference
    };
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(numerical("Poisson deviance is not finitely representable"))
    }
}

fn one_plus_x_log1p_minus_x(value: f64) -> f64 {
    if value.abs() > 1e-4 {
        return (1.0 + value) * value.ln_1p() - value;
    }
    // (1+x)log(1+x)-x = sum_{k=2}^inf (-1)^k x^k/[k(k-1)].
    // Twelve terms leave the truncation far below one ulp at this cutoff.
    let mut power = value * value;
    let mut result = 0.5 * power;
    for order in 3_i32..=12 {
        power *= value;
        let sign = if order % 2 == 0 { 1.0 } else { -1.0 };
        result += sign * power / f64::from(order * (order - 1));
    }
    result
}

pub(super) fn log_gamma(x: f64) -> f64 {
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let shifted = x - 1.0;
    let mut series = C[0];
    for (index, coefficient) in C.iter().enumerate().skip(1) {
        series += coefficient / (shifted + index as f64);
    }
    let t = shifted + 7.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (shifted + 0.5) * t.ln() - t + series.ln()
}
