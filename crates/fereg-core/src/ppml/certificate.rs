use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;

#[derive(Debug, Clone, PartialEq)]
pub struct PpmlCertificate {
    pub finite_absolute_infinity_norm: f64,
    pub fixed_effect_absolute_infinity_norm: f64,
    pub relative_infinity_norm: f64,
    pub relative_two_norm: f64,
    pub passed: bool,
}

pub(crate) fn score_certificate(
    outcome: &[f64],
    mean: &[f64],
    prior_weights: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    tolerance: f64,
) -> Result<PpmlCertificate> {
    let nrows = outcome.len();
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML score tolerance must be finite and positive",
        ));
    }
    if mean.len() != nrows
        || prior_weights.len() != nrows
        || regressors.nrows() != nrows
        || fixed_effects.is_some_and(|design| design.nobs() != nrows)
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML score-certificate inputs have inconsistent dimensions",
        ));
    }

    let mut residual_scale_square = 0.0_f64;
    let mut reference_scale_square = 0.0_f64;
    for row in 0..nrows {
        let residual = outcome[row] - mean[row];
        let inverse_variance_weight = prior_weights[row] / mean[row];
        residual_scale_square =
            inverse_variance_weight.mul_add(residual * residual, residual_scale_square);
        reference_scale_square =
            inverse_variance_weight.mul_add(outcome[row] * outcome[row], reference_scale_square);
    }
    // The outcome reference prevents an almost exact fit from turning harmless
    // floating-point cancellation into an order-one relative score.
    let score_scale = residual_scale_square
        .sqrt()
        .max(reference_scale_square.sqrt());
    if !score_scale.is_finite() {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML score scale overflowed",
        ));
    }

    let mut finite_absolute = 0.0_f64;
    let mut fixed_effect_absolute = 0.0_f64;
    let mut relative_infinity = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
    for column in 0..regressors.ncols() {
        let mut moment = 0.0_f64;
        let mut basis_square = 0.0_f64;
        for row in 0..nrows {
            let value = regressors[(row, column)];
            moment = (prior_weights[row] * value).mul_add(outcome[row] - mean[row], moment);
            basis_square = (prior_weights[row] * mean[row] * value).mul_add(value, basis_square);
        }
        finite_absolute = finite_absolute.max(moment.abs());
        add_relative(
            moment,
            basis_square.sqrt(),
            score_scale,
            &mut relative_infinity,
            &mut relative_square_sum,
            &mut relative_count,
        );
    }

    if let Some(design) = fixed_effects {
        for effect in design.effects() {
            let channels = effect.channels();
            let directions = effect.n_levels().saturating_mul(channels);
            let mut moments = vec![0.0_f64; directions];
            let mut basis_squares = vec![0.0_f64; directions];
            for row in 0..nrows {
                let level = effect.codes()[row] as usize;
                for channel in 0..channels {
                    let direction = level * channels + channel;
                    let basis = effect.basis_value(channel, row);
                    moments[direction] = (prior_weights[row] * basis)
                        .mul_add(outcome[row] - mean[row], moments[direction]);
                    basis_squares[direction] = (prior_weights[row] * mean[row] * basis)
                        .mul_add(basis, basis_squares[direction]);
                }
            }
            for direction in 0..directions {
                fixed_effect_absolute = fixed_effect_absolute.max(moments[direction].abs());
                add_relative(
                    moments[direction],
                    basis_squares[direction].sqrt(),
                    score_scale,
                    &mut relative_infinity,
                    &mut relative_square_sum,
                    &mut relative_count,
                );
            }
        }
    }

    let relative_two = if relative_count == 0 {
        0.0
    } else {
        (relative_square_sum / relative_count as f64).sqrt()
    };
    if !finite_absolute.is_finite()
        || !fixed_effect_absolute.is_finite()
        || !relative_infinity.is_finite()
        || !relative_two.is_finite()
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML score certificate became nonfinite",
        ));
    }
    Ok(PpmlCertificate {
        finite_absolute_infinity_norm: finite_absolute,
        fixed_effect_absolute_infinity_norm: fixed_effect_absolute,
        relative_infinity_norm: relative_infinity,
        relative_two_norm: relative_two,
        passed: relative_infinity <= tolerance && relative_two <= tolerance,
    })
}

fn add_relative(
    moment: f64,
    basis_norm: f64,
    score_scale: f64,
    infinity: &mut f64,
    square_sum: &mut f64,
    count: &mut usize,
) {
    let denominator = basis_norm * score_scale;
    let relative = if denominator > 0.0 {
        moment.abs() / denominator
    } else if moment == 0.0 {
        0.0
    } else {
        f64::INFINITY
    };
    *infinity = (*infinity).max(relative);
    *square_sum = relative.mul_add(relative, *square_sum);
    *count += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn exact_fit_uses_outcome_reference_scale() {
        let x = DenseMatrix::from_columns(&[vec![1.0; 4]]).expect("matrix");
        let y = [1.0, 2.0, 3.0, 4.0];
        let mean = [1.0 + 1e-15, 2.0 - 1e-15, 3.0 + 1e-15, 4.0 - 1e-15];
        let certificate =
            score_certificate(&y, &mean, &[1.0; 4], &x, None, 1e-12).expect("certificate");
        assert!(certificate.passed);
    }

    #[test]
    fn nonzero_fe_score_is_rejected() {
        let x = DenseMatrix::zeros(4, 0).expect("matrix");
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![]).expect("effect");
        let design = FeDesign::new(4, vec![effect]).expect("design");
        let certificate = score_certificate(
            &[2.0, 2.0, 1.0, 1.0],
            &[1.0; 4],
            &[1.0; 4],
            &x,
            Some(&design),
            1e-8,
        )
        .expect("certificate");
        assert!(!certificate.passed);
    }
}
