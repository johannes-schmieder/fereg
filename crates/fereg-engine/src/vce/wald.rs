//! Canonical tested coordinates; singular covariance gives an undefined test.
use super::{invalid, numerical};
use crate::{
    data::DenseMatrix,
    error::{reserved, Result},
    spec::Estimator,
    threads::compensated_sum,
};
#[derive(Debug, Clone, PartialEq)]
pub struct Wald {
    pub statistic: Option<f64>,
    pub degrees_of_freedom: usize,
    pub tested: Vec<usize>,
    pub estimator: Estimator,
    pub solve_error: f64,
}
pub fn wald(
    coefficients: &[f64],
    covariance: &DenseMatrix,
    tested: &[usize],
    estimator: Estimator,
    tolerance: f64,
) -> Result<Wald> {
    let p = coefficients.len();
    if covariance.nrows() != p
        || covariance.ncols() != p
        || !tolerance.is_finite()
        || tolerance <= 0.
        || tolerance >= 1.
        || coefficients.iter().any(|v| !v.is_finite())
        || covariance.as_slice().iter().any(|v| !v.is_finite())
        || tested.iter().any(|j| *j >= p)
        || tested.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(invalid("Wald coordinates or covariance are invalid"));
    }
    let n = tested.len();
    let mut positions = reserved(n)?;
    positions.extend_from_slice(tested);
    let mut report = Wald {
        statistic: None,
        degrees_of_freedom: n,
        tested: positions,
        estimator,
        solve_error: 0.,
    };
    if n == 0 {
        return Ok(report);
    }
    let mut lower = DenseMatrix::zeros(n, n)?;
    let scale = tested
        .iter()
        .map(|j| covariance[(*j, *j)].abs())
        .fold(f64::MIN_POSITIVE, f64::max);
    for i in 0..n {
        for j in 0..=i {
            let value = covariance[(tested[i], tested[j])]
                - compensated_sum((0..j).map(|k| lower[(i, k)] * lower[(j, k)]));
            if i == j {
                if value < -tolerance * scale {
                    return Err(numerical("Wald covariance is indefinite"));
                }
                if value <= tolerance * scale {
                    return Ok(report);
                }
                lower[(i, j)] = value.sqrt();
            } else {
                lower[(i, j)] = value / lower[(j, j)];
            }
        }
    }
    let mut solution = reserved(n)?;
    solution.extend(tested.iter().map(|j| coefficients[*j]));
    for i in 0..n {
        solution[i] = (solution[i] - compensated_sum((0..i).map(|j| lower[(i, j)] * solution[j])))
            / lower[(i, i)];
    }
    for i in (0..n).rev() {
        solution[i] = (solution[i]
            - compensated_sum((i + 1..n).map(|j| lower[(j, i)] * solution[j])))
            / lower[(i, i)];
    }
    let quadratic = compensated_sum((0..n).map(|i| coefficients[tested[i]] * solution[i]));
    let mut error = 0_f64;
    let mut residual = 0_f64;
    let mut reference = f64::MIN_POSITIVE;
    for i in 0..n {
        let rhs = coefficients[tested[i]];
        let fitted =
            compensated_sum((0..n).map(|j| covariance[(tested[i], tested[j])] * solution[j]));
        let scale = rhs.abs().max(1.)
            + compensated_sum(
                (0..n).map(|j| (covariance[(tested[i], tested[j])] * solution[j]).abs()),
            );
        error = error.max((fitted - rhs).abs() / scale);
        residual = residual.max((fitted - rhs).abs());
        reference = reference.max(fitted.abs()).max(rhs.abs());
    }
    report.solve_error = if estimator == Estimator::Ols {
        error
    } else {
        residual / reference
    };
    if !quadratic.is_finite()
        || quadratic < 0.
        || !report.solve_error.is_finite()
        || report.solve_error > tolerance
    {
        return Err(numerical("Wald solve certificate failed"));
    }
    report.statistic = Some(if estimator == Estimator::Ols {
        quadratic / n as f64
    } else {
        quadratic
    });
    Ok(report)
}

/// The standardized coefficient is shared; callers choose t or z reference laws.
pub(crate) fn coefficient_statistics(
    coefficients: &[f64],
    errors: &[f64],
    coordinates: &[usize],
) -> Result<Vec<f64>> {
    if coefficients.len() != errors.len()
        || coordinates.iter().any(|&slot| slot >= coefficients.len())
    {
        return Err(invalid("coefficient statistic coordinates disagree"));
    }
    let mut values = reserved(coefficients.len())?;
    values.resize(coefficients.len(), f64::NAN);
    for &slot in coordinates {
        if errors[slot] > 0. {
            let value = coefficients[slot] / errors[slot];
            if !value.is_finite() {
                return Err(numerical("coefficient statistic is not representable"));
            }
            values[slot] = value;
        }
    }
    Ok(values)
}
