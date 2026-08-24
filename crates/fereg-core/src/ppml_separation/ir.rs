use super::{DetectionResult, MethodStatus, SeparationCertificate};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;

use super::projection::{project_onto_model, weighted_norm, ProjectionConfig};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IteratedRectifierConfig {
    pub projection: ProjectionConfig,
    pub tolerance: f64,
    pub certificate_tolerance: f64,
    pub max_iterations: usize,
    pub strict: bool,
}

pub(crate) fn iterated_rectifier(
    outcome: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    config: IteratedRectifierConfig,
) -> Result<DetectionResult> {
    let IteratedRectifierConfig {
        projection,
        tolerance,
        certificate_tolerance,
        max_iterations,
        strict,
    } = config;
    let boundary = outcome
        .iter()
        .enumerate()
        .filter_map(|(row, value)| (*value == 0.0).then_some(row))
        .collect::<Vec<_>>();
    if boundary.is_empty() {
        return Ok(DetectionResult::empty(
            MethodStatus::NoCandidates,
            "no zero-outcome observations",
        ));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 || tolerance >= 1.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "iterated-rectifier tolerance must lie strictly between zero and one",
        ));
    }
    if max_iterations == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "iterated-rectifier iteration budget must be positive",
        ));
    }

    // The reference method uses K=N0/tol^2 on interior observations.  Divide
    // all weights by K to avoid huge floating-point weights while preserving
    // the identical weighted least-squares problem.
    let boundary_weight = (tolerance * tolerance / boundary.len() as f64).max(f64::MIN_POSITIVE);
    let weights = outcome
        .iter()
        .map(|value| if *value > 0.0 { 1.0 } else { boundary_weight })
        .collect::<Vec<_>>();
    let mut u = outcome
        .iter()
        .map(|value| if *value == 0.0 { 1.0 } else { 0.0 })
        .collect::<Vec<_>>();

    for iteration in 1..=max_iterations {
        let projected = project_onto_model(&u, regressors, &weights, fixed_effects, projection)?;
        let z = projected.fitted;
        let scale = z
            .iter()
            .fold(1.0_f64, |maximum, value| maximum.max(value.abs()));
        let zero_threshold = tolerance * scale;
        let interior_zero = outcome
            .iter()
            .zip(&z)
            .filter(|(outcome, _)| **outcome > 0.0)
            .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()));
        let minimum_boundary = boundary
            .iter()
            .map(|row| z[*row])
            .fold(f64::INFINITY, f64::min);

        if minimum_boundary >= -zero_threshold {
            let positive_maximum = boundary.iter().map(|row| z[*row]).fold(0.0_f64, f64::max);
            if positive_maximum <= zero_threshold {
                let certificate = SeparationCertificate {
                    interior_zero_relative: interior_zero / scale,
                    boundary_violation_relative: (-minimum_boundary).max(0.0) / scale,
                    span_residual_relative: 0.0,
                    positive_maximum,
                    passed: interior_zero <= certificate_tolerance * scale
                        && (-minimum_boundary).max(0.0) <= certificate_tolerance * scale,
                };
                if !certificate.passed {
                    let message = format!(
                        "iterated-rectifier zero direction failed certification: interior={:.3e}, boundary={:.3e}",
                        certificate.interior_zero_relative,
                        certificate.boundary_violation_relative
                    );
                    if strict {
                        return Err(FeregError::new(ErrorCode::InvalidData, message));
                    }
                    return Ok(DetectionResult {
                        dropped: Vec::new(),
                        iterations: iteration,
                        status: MethodStatus::Uncertified,
                        certificate: Some(certificate),
                        note: message,
                    });
                }
                return Ok(DetectionResult {
                    dropped: Vec::new(),
                    iterations: iteration,
                    status: MethodStatus::NoSeparation,
                    certificate: Some(certificate),
                    note: "iterated rectifier converged to the zero certificate".to_owned(),
                });
            }

            // A fitted value is mathematically in the model column space.  A
            // second independent projection makes that numerical claim explicit.
            let span_check =
                project_onto_model(&z, regressors, &weights, fixed_effects, projection)?;
            let span_denominator = weighted_norm(&z, &weights)?.max(f64::MIN_POSITIVE);
            let span_residual_relative =
                weighted_norm(&span_check.residual, &weights)? / span_denominator;
            let boundary_violation = (-minimum_boundary).max(0.0) / scale;
            let interior_zero_relative = interior_zero / scale;
            let passed = span_check.certificate.passed
                && span_residual_relative <= certificate_tolerance
                && interior_zero_relative <= certificate_tolerance
                && boundary_violation <= certificate_tolerance;
            let certificate = SeparationCertificate {
                interior_zero_relative,
                boundary_violation_relative: boundary_violation,
                span_residual_relative,
                positive_maximum,
                passed,
            };
            if !passed {
                let message = format!(
                    "iterated-rectifier candidate failed certification: interior={interior_zero_relative:.3e}, boundary={boundary_violation:.3e}, span={span_residual_relative:.3e}"
                );
                if strict {
                    return Err(FeregError::new(ErrorCode::InvalidData, message));
                }
                return Ok(DetectionResult {
                    dropped: Vec::new(),
                    iterations: iteration,
                    status: MethodStatus::Uncertified,
                    certificate: Some(certificate),
                    note: message,
                });
            }
            let dropped = boundary
                .iter()
                .copied()
                .filter(|row| z[*row] > zero_threshold)
                .collect::<Vec<_>>();
            return Ok(DetectionResult {
                dropped,
                iterations: iteration,
                status: MethodStatus::Certified,
                certificate: Some(certificate),
                note: "iterated-rectifier separation certificate passed".to_owned(),
            });
        }

        let mut maximum = 0.0_f64;
        for row in 0..u.len() {
            u[row] = if outcome[row] > 0.0 || z[row].abs() <= zero_threshold {
                0.0
            } else {
                z[row].max(0.0)
            };
            maximum = maximum.max(u[row]);
        }
        if maximum > 0.0 {
            u.iter_mut().for_each(|value| *value /= maximum);
        }
    }

    let message =
        format!("iterated-rectifier separation check exhausted {max_iterations} iterations");
    if strict {
        Err(FeregError::new(ErrorCode::InvalidData, message))
    } else {
        Ok(DetectionResult {
            dropped: Vec::new(),
            iterations: max_iterations,
            status: MethodStatus::MaximumIterations,
            certificate: None,
            note: message,
        })
    }
}
