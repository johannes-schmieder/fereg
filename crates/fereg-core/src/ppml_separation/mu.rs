use super::projection::{
    project_onto_model, weighted_norm, ProjectionConfig, LEGACY_PROJECTION_RANK_TOLERANCE,
};
use super::subset::{select_rows, SubsetProblem};
use super::{
    DetectionProposal, MethodStatus, SeparationCertificate, SeparationConfig, SeparationMethod,
};
use crate::error::{ErrorCode, FeregError, Result};
use crate::ols::FiniteSolver;
use crate::ppml::{means_from_eta, PpmlConfig, PpmlStart};

pub(super) fn detect_mu(
    subset: &SubsetProblem,
    ppml: PpmlConfig,
    config: SeparationConfig,
    start: Option<&PpmlStart>,
    original_active: &[bool],
) -> Result<DetectionProposal> {
    let Some(start) = start else {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Mu,
            MethodStatus::SkippedUnsupported,
            "mu separation requires a supplied warm-start predictor",
        ));
    };
    if start.linear_predictor.len() != original_active.len() {
        return Err(FeregError::new(
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
        return Ok(DetectionProposal::empty(
            SeparationMethod::Mu,
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
        return Ok(DetectionProposal::empty(
            SeparationMethod::Mu,
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
            rank_tolerance: LEGACY_PROJECTION_RANK_TOLERANCE,
            collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
            certificate_tolerance: config.projection_tolerance,
        },
    )?;
    let positive_direction = projection.fitted;
    let z = positive_direction
        .iter()
        .map(|value| -*value)
        .collect::<Vec<_>>();
    let scale = z
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    if !scale.is_finite() || scale <= f64::MIN_POSITIVE {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Mu,
            MethodStatus::Uncertified,
            "mu projection returned a zero direction",
        ));
    }
    let threshold = config.tolerance * scale;
    let certificate_threshold = config.certificate_tolerance * scale;
    let interior_zero = subset
        .outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome > 0.0)
        .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()));
    let positive_maximum = subset
        .outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| *value)
        .fold(f64::NEG_INFINITY, f64::max);
    let span_check = project_onto_model(
        &z,
        &subset.regressors,
        &weights,
        subset.fixed_effects.as_ref(),
        ProjectionConfig {
            absorb: ppml.absorb,
            finite_solver: FiniteSolver::Auto,
            rank_tolerance: LEGACY_PROJECTION_RANK_TOLERANCE,
            collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
            certificate_tolerance: config.projection_tolerance,
        },
    )?;
    let span_residual_relative = weighted_norm(&span_check.residual, &weights)?
        / weighted_norm(&z, &weights)?.max(f64::MIN_POSITIVE);
    let dropped = subset
        .outcome
        .iter()
        .zip(&z)
        .enumerate()
        .filter_map(|(row, (outcome, value))| {
            (*outcome == 0.0 && *value < -threshold).then_some(row)
        })
        .collect::<Vec<_>>();
    let negative_margin_relative = dropped
        .iter()
        .map(|row| -z[*row] / scale)
        .fold(f64::INFINITY, f64::min);
    let certificate = SeparationCertificate {
        proof: crate::ppml_separation::SeparationCertificateProof::RecessionDirection,
        interior_zero_relative: interior_zero / scale,
        boundary_violation_relative: positive_maximum.max(0.0) / scale,
        span_residual_relative,
        negative_margin_relative,
        direction_scale: scale,
        positive_maximum,
        passed: projection.certificate.passed
            && span_check.certificate.passed
            && span_residual_relative <= config.certificate_tolerance
            && interior_zero <= certificate_threshold
            && positive_maximum <= certificate_threshold
            && negative_margin_relative >= config.certificate_tolerance,
    };
    if !certificate.passed {
        let note = format!(
            "low means did not yield a certified separation direction: interior={:.3e}, boundary={:.3e}, span={:.3e}",
            certificate.interior_zero_relative,
            certificate.boundary_violation_relative,
            certificate.span_residual_relative,
        );
        if config.strict {
            return Err(FeregError::new(ErrorCode::NonConvergence, note));
        }
        return Ok(DetectionProposal {
            method: SeparationMethod::Mu,
            dropped: Vec::new(),
            iterations: 1,
            status: MethodStatus::Uncertified,
            certificate: Some(certificate),
            exact_fe_certificate: None,
            direction: None,
            note,
        });
    }
    Ok(DetectionProposal {
        method: SeparationMethod::Mu,
        dropped,
        iterations: 1,
        status: MethodStatus::Certified,
        certificate: Some(certificate),
        exact_fe_certificate: None,
        direction: Some(z),
        note: "mu candidates passed an independent column-space/sign certificate".to_owned(),
    })
}
