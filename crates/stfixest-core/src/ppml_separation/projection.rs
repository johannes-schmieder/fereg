use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::{
    absorb_fixed_effects, certify_residuals_with_reference_norms, AbsorbConfig, FeCertificate,
    FeDesign, SolveReport,
};
use crate::ols::{solve_rank_revealing, FiniteSolver};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ProjectionConfig {
    pub absorb: AbsorbConfig,
    pub finite_solver: FiniteSolver,
    pub rank_tolerance: f64,
    pub collinearity_tolerance: f64,
    pub certificate_tolerance: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProjectionCertificate {
    pub finite_relative_infinity_norm: f64,
    pub fixed_effect: FeCertificate,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProjectionResult {
    pub fitted: Vec<f64>,
    pub residual: Vec<f64>,
    pub finite_rank: usize,
    pub absorb_report: Option<SolveReport>,
    pub certificate: ProjectionCertificate,
}

pub(crate) fn project_onto_model(
    outcome: &[f64],
    regressors: &DenseMatrix,
    weights: &[f64],
    fixed_effects: Option<&FeDesign>,
    config: ProjectionConfig,
) -> Result<ProjectionResult> {
    validate(outcome, regressors, weights, fixed_effects, config)?;
    let nrows = outcome.len();
    let ncols = regressors.ncols();
    let outcome_norm = weighted_norm(outcome, weights)?;
    let original_norms = (0..ncols)
        .map(|column| weighted_matrix_column_norm(regressors, column, weights))
        .collect::<Result<Vec<_>>>()?;

    let mut columns = Vec::with_capacity(ncols + 1);
    columns.push(outcome.to_vec());
    for column in 0..ncols {
        columns.push(regressors.column(column)?);
    }
    let mut transformed = DenseMatrix::from_columns(&columns)?;
    let absorb_report = if let Some(design) = fixed_effects {
        if design.effects().is_empty() {
            None
        } else {
            let mut absorb = config.absorb;
            absorb.map.tolerance = config.certificate_tolerance;
            absorb.lsmr.tolerance = config.certificate_tolerance;
            let report = absorb_fixed_effects(design, weights, &mut transformed, absorb)?;
            if !report.certified() {
                return Err(StFixestError::new(
                    ErrorCode::InvalidData,
                    format!(
                        "separation projection failed its FE certificate: {:?}",
                        report.stop_reason
                    ),
                ));
            }
            Some(report)
        }
    } else {
        None
    };

    let within_outcome = transformed.column(0)?;
    let mut candidate_columns = Vec::new();
    for column in 0..ncols {
        let values = transformed.column(column + 1)?;
        let norm = weighted_norm(&values, weights)?;
        if norm > config.collinearity_tolerance * original_norms[column].max(1.0) {
            candidate_columns.push(values);
        }
    }
    let candidates = if candidate_columns.is_empty() {
        DenseMatrix::zeros(nrows, 0)?
    } else {
        DenseMatrix::from_columns(&candidate_columns)?
    };
    let solve = solve_rank_revealing(
        &candidates,
        &within_outcome,
        weights,
        config.rank_tolerance,
        config.finite_solver,
    )?;

    let mut residual = within_outcome;
    for row in 0..nrows {
        let mut fitted = 0.0_f64;
        for &candidate in &solve.kept {
            fitted = candidates[(row, candidate)].mul_add(solve.coefficients[candidate], fitted);
        }
        residual[row] -= fitted;
    }
    let fitted = outcome
        .iter()
        .zip(&residual)
        .map(|(value, residual)| value - residual)
        .collect::<Vec<_>>();

    let residual_matrix = DenseMatrix::from_columns(&[residual.clone()])?;
    let empty_design;
    let design = if let Some(design) = fixed_effects {
        design
    } else {
        empty_design = FeDesign::new(nrows, Vec::new())?;
        &empty_design
    };
    let fixed_effect = certify_residuals_with_reference_norms(
        design,
        weights,
        &residual_matrix,
        &[outcome_norm],
        config.certificate_tolerance,
    )?;
    let residual_scale = weighted_norm(&residual, weights)?.max(outcome_norm);
    let mut finite_relative = 0.0_f64;
    for column in 0..ncols {
        let mut moment = 0.0_f64;
        for row in 0..nrows {
            moment = (weights[row] * regressors[(row, column)]).mul_add(residual[row], moment);
        }
        let denominator = original_norms[column] * residual_scale;
        let relative = if denominator > 0.0 {
            moment.abs() / denominator
        } else if moment == 0.0 {
            0.0
        } else {
            f64::INFINITY
        };
        finite_relative = finite_relative.max(relative);
    }
    if !finite_relative.is_finite() {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "separation projection certificate became nonfinite",
        ));
    }
    let passed = finite_relative <= config.certificate_tolerance && fixed_effect.passed;
    let certificate = ProjectionCertificate {
        finite_relative_infinity_norm: finite_relative,
        fixed_effect,
        passed,
    };
    if !certificate.passed {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            format!(
                "separation projection did not certify: finite={:.3e}, FE={:.3e}",
                certificate.finite_relative_infinity_norm,
                certificate.fixed_effect.relative_infinity_norm
            ),
        ));
    }

    Ok(ProjectionResult {
        fitted,
        residual,
        finite_rank: solve.rank,
        absorb_report,
        certificate,
    })
}

fn validate(
    outcome: &[f64],
    regressors: &DenseMatrix,
    weights: &[f64],
    fixed_effects: Option<&FeDesign>,
    config: ProjectionConfig,
) -> Result<()> {
    if outcome.len() != regressors.nrows()
        || outcome.len() != weights.len()
        || fixed_effects.is_some_and(|design| design.nobs() != outcome.len())
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "separation projection inputs have inconsistent dimensions",
        ));
    }
    if outcome.iter().any(|value| !value.is_finite())
        || regressors.as_slice().iter().any(|value| !value.is_finite())
        || weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "separation projection requires finite values and positive weights",
        ));
    }
    for value in [
        config.rank_tolerance,
        config.collinearity_tolerance,
        config.certificate_tolerance,
    ] {
        if !value.is_finite() || value <= 0.0 || value >= 1.0 {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "separation projection tolerances must lie strictly between zero and one",
            ));
        }
    }
    Ok(())
}

pub(crate) fn weighted_norm(values: &[f64], weights: &[f64]) -> Result<f64> {
    let mut square_sum = 0.0_f64;
    for (value, weight) in values.iter().zip(weights) {
        square_sum = (*weight * *value).mul_add(*value, square_sum);
    }
    let norm = square_sum.sqrt();
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(StFixestError::new(
            ErrorCode::InvalidData,
            "weighted norm overflowed during separation",
        ))
    }
}

fn weighted_matrix_column_norm(
    matrix: &DenseMatrix,
    column: usize,
    weights: &[f64],
) -> Result<f64> {
    let mut square_sum = 0.0_f64;
    for row in 0..matrix.nrows() {
        let value = matrix[(row, column)];
        square_sum = (weights[row] * value).mul_add(value, square_sum);
    }
    let norm = square_sum.sqrt();
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(StFixestError::new(
            ErrorCode::InvalidData,
            "weighted column norm overflowed during separation",
        ))
    }
}
