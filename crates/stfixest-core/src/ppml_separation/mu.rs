use super::projection::{project_onto_model, weighted_norm, ProjectionConfig};
use super::subset::{select_rows, SubsetProblem};
use super::{DetectionResult, MethodStatus, SeparationCertificate, SeparationConfig};
use crate::error::{ErrorCode, Result, StFixestError};
use crate::ols::FiniteSolver;
use crate::ppml::{means_from_eta, PpmlConfig, PpmlStart};

pub(super) fn detect_mu(
    subset: &SubsetProblem,
    ppml: PpmlConfig,
    config: SeparationConfig,
    start: Option<&PpmlStart>,
    original_active: &[bool],
) -> Result<DetectionResult> {
    let Some(start) = start else {
        return Ok(DetectionResult::empty(
            MethodStatus::SkippedUnsupported,
            "mu separation requires a supplied warm-start predictor",
        ));
    };
    if start.linear_predictor.len() != original_active.len() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "mu separation warm start has the wrong observation count",
        ));
    }
    let eta = select_rows(&start.linear_predictor, &subset.original_rows);
    let mut mean = vec![0.0_f64; eta.len()];
    means_from_eta(&eta, ppml.bounds, &mut mean)?;
    let minimum_positive = subset
        .outcome
        .iter()
        .zip(&mean)
        .filter_map(|(outcome, mean)| (*outcome > 0.0).then_some(*mean))
        .fold(f64::INFINITY, f64::min);
    if !minimum_positive.is_finite() {
        return Ok(DetectionResult::empty(
            MethodStatus::NoCandidates,
            "mu method found no positive-outcome reference mean",
        ));
    }
    let candidate = subset
        .outcome
        .iter()
        .zip(&mean)
        .map(|(outcome, mean)| {
            if *outcome == 0.0 && *mean <= config.mu_tolerance * minimum_positive {
                1.0
            } else {
                0.0
            }
        })
        .collect::<Vec<_>>();
    if candidate.iter().all(|value| *value == 0.0) {
        return Ok(DetectionResult::empty(
            MethodStatus::NoCandidates,
            "mu method found no sufficiently small zero-outcome means",
        ));
    }
    let weights = vec![1.0_f64; candidate.len()];
    let projection = project_onto_model(
        &candidate,
        &subset.regressors,
        &weights,
        subset.fixed_effects.as_ref(),
        ProjectionConfig {
            absorb: ppml.absorb,
            finite_solver: FiniteSolver::Auto,
            rank_tolerance: ppml.rank_tolerance,
            collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
            certificate_tolerance: config.projection_tolerance,
        },
    )?;
    let z = projection.fitted;
    let scale = z
        .iter()
        .fold(1.0_f64, |maximum, value| maximum.max(value.abs()));
    let threshold = config.certificate_tolerance * scale;
    let interior_zero = subset
        .outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome > 0.0)
        .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()));
    let minimum_boundary = subset
        .outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| *value)
        .fold(f64::INFINITY, f64::min);
    let positive_maximum = subset
        .outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| *value)
        .fold(0.0_f64, f64::max);
    let span_check = project_onto_model(
        &z,
        &subset.regressors,
        &weights,
        subset.fixed_effects.as_ref(),
        ProjectionConfig {
            absorb: ppml.absorb,
            finite_solver: FiniteSolver::Auto,
            rank_tolerance: ppml.rank_tolerance,
            collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
            certificate_tolerance: config.projection_tolerance,
        },
    )?;
    let span_residual_relative = weighted_norm(&span_check.residual, &weights)?
        / weighted_norm(&z, &weights)?.max(f64::MIN_POSITIVE);
    let certificate = SeparationCertificate {
        interior_zero_relative: interior_zero / scale,
        boundary_violation_relative: (-minimum_boundary).max(0.0) / scale,
        span_residual_relative,
        positive_maximum,
        passed: projection.certificate.passed
            && span_check.certificate.passed
            && span_residual_relative <= config.certificate_tolerance
            && interior_zero <= threshold
            && minimum_boundary >= -threshold
            && positive_maximum > threshold,
    };
    if !certificate.passed {
        let note = format!(
            "low means did not yield a certified separation direction: interior={:.3e}, boundary={:.3e}, span={:.3e}",
            certificate.interior_zero_relative,
            certificate.boundary_violation_relative,
            certificate.span_residual_relative,
        );
        if config.strict {
            return Err(StFixestError::new(ErrorCode::InvalidData, note));
        }
        return Ok(DetectionResult {
            dropped: Vec::new(),
            iterations: 1,
            status: MethodStatus::Uncertified,
            certificate: Some(certificate),
            note,
        });
    }
    let dropped = subset
        .outcome
        .iter()
        .zip(&z)
        .enumerate()
        .filter_map(|(row, (outcome, value))| {
            (*outcome == 0.0 && *value > threshold).then_some(row)
        })
        .collect();
    Ok(DetectionResult {
        dropped,
        iterations: 1,
        status: MethodStatus::Certified,
        certificate: Some(certificate),
        note: "mu candidates passed an independent column-space/sign certificate".to_owned(),
    })
}
