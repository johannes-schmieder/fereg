//! Reconstructed finite residuals, normalized scores and retained inverse check.
use super::{Candidate, Policy, Problem, Products};
use crate::{
    data::DenseMatrix,
    error::{reserved, Result},
    linalg::ColumnScaling,
    threads::compensated_sum,
};
pub(super) enum CandidateCheck {
    Pass(Products),
    Fail { score: f64, inverse: f64 },
}
pub(super) fn assess(
    problem: &Problem<'_>,
    columns: &[usize],
    scaling: &ColumnScaling,
    gram: &DenseMatrix,
    candidate: &Candidate,
    policy: Policy,
) -> Result<CandidateCheck> {
    if candidate
        .coefficients
        .as_slice()
        .iter()
        .chain(candidate.scaled_bread.as_slice())
        .any(|v| !v.is_finite())
    {
        return Ok(CandidateCheck::Fail {
            score: f64::INFINITY,
            inverse: f64::INFINITY,
        });
    }
    let mut coefficients = reserved(problem.slots.len())?;
    coefficients.resize(problem.slots.len(), 0.);
    for (local, &column) in columns.iter().enumerate() {
        coefficients[column] = candidate.coefficients[(local, 0)];
    }
    let (fitted, residuals) = reconstruct(problem, columns, &candidate.coefficients)?;
    if fitted.iter().chain(&residuals).any(|v| !v.is_finite()) {
        return Ok(CandidateCheck::Fail {
            score: f64::INFINITY,
            inverse: f64::INFINITY,
        });
    }
    let outcome_scale = problem
        .outcome
        .iter()
        .chain(&residuals)
        .map(|v| v.abs())
        .fold(0., f64::max);
    let mut reference = 0_f64;
    let mut residual_norm = 0_f64;
    if outcome_scale > 0. {
        for (row, &residual) in residuals.iter().enumerate() {
            reference =
                reference.hypot(scaling.roots[row] * (problem.outcome[row] / outcome_scale));
            residual_norm = residual_norm.hypot(scaling.roots[row] * (residual / outcome_scale));
        }
    }
    let denominator = reference.max(residual_norm);
    let mut score = 0_f64;
    if denominator > 0. {
        for &local in &candidate.kept {
            let column = columns[local];
            if scaling.norms[local] == 0. {
                continue;
            }
            let moment = compensated_sum((0..problem.outcome.len()).map(|row| {
                let z = (problem.design[(row, column)] / scaling.maxima[local])
                    * scaling.roots[row]
                    / scaling.norms[local];
                z * scaling.roots[row] * (residuals[row] / outcome_scale)
            }));
            if !moment.is_finite() {
                return Ok(CandidateCheck::Fail {
                    score: f64::INFINITY,
                    inverse: f64::INFINITY,
                });
            }
            score = score.max(moment.abs() / denominator / gram[(local, local)].sqrt());
        }
    }
    let inverse_error = inverse_error(gram, &candidate.scaled_bread, &candidate.kept);
    if !score.is_finite()
        || score > policy.score_tolerance
        || inverse_error > policy.bread_tolerance
    {
        return Ok(CandidateCheck::Fail {
            score,
            inverse: inverse_error,
        });
    }
    let mut bread = DenseMatrix::zeros(problem.slots.len(), problem.slots.len())?;
    for &left in &candidate.kept {
        for &right in &candidate.kept {
            let value = scaling.normal_coordinate(candidate.scaled_bread[(left, right)], left);
            bread[(columns[left], columns[right])] = scaling.normal_coordinate(value, right);
        }
    }
    if bread.as_slice().iter().any(|v| !v.is_finite()) {
        return Ok(CandidateCheck::Fail {
            score: f64::INFINITY,
            inverse: f64::INFINITY,
        });
    }
    Ok(CandidateCheck::Pass(Products {
        coefficients,
        bread,
        fitted,
        residuals,
        score,
        inverse_error,
    }))
}

// Legacy's two-sided, component-scaled backward-error test; an ill-conditioned
// retained inverse still needs independent forward-accuracy oracle coverage.
fn inverse_error(gram: &DenseMatrix, bread: &DenseMatrix, kept: &[usize]) -> f64 {
    let mut maximum = 0_f64;
    for reverse in [false, true] {
        for &row in kept {
            let mut error = 0.;
            let mut scale = 1.;
            for &column in kept {
                let term = |inner: &usize| {
                    if reverse {
                        bread[(row, *inner)] * gram[(*inner, column)]
                    } else {
                        gram[(row, *inner)] * bread[(*inner, column)]
                    }
                };
                let value = compensated_sum(kept.iter().map(term));
                let absolute = compensated_sum(kept.iter().map(|inner| term(inner).abs()));
                error += (value - if row == column { 1. } else { 0. }).abs();
                scale += absolute;
            }
            if !error.is_finite() || !scale.is_finite() {
                return f64::INFINITY;
            }
            maximum = maximum.max(error / scale);
        }
    }
    maximum
}

pub(super) fn reconstruct(
    problem: &Problem<'_>,
    columns: &[usize],
    coefficients: &DenseMatrix,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut fitted = reserved(problem.outcome.len())?;
    let mut residuals = reserved(problem.outcome.len())?;
    for row in 0..problem.outcome.len() {
        let value = compensated_sum(
            columns
                .iter()
                .enumerate()
                .map(|(local, &column)| problem.design[(row, column)] * coefficients[(local, 0)]),
        );
        fitted.push(value);
        residuals.push(problem.outcome[row] - value);
    }
    Ok((fitted, residuals))
}
