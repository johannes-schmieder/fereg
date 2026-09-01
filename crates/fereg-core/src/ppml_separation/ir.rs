use super::{DetectionProposal, MethodStatus, SeparationCertificate, SeparationMethod};
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
    /// Retained for compatibility with the stale internal caller.  The
    /// production coordinator always treats nonconvergence as fatal.
    pub strict: bool,
}

pub(crate) fn iterated_rectifier(
    outcome: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    config: IteratedRectifierConfig,
) -> Result<DetectionProposal> {
    let IteratedRectifierConfig {
        projection,
        tolerance,
        certificate_tolerance,
        max_iterations,
        strict: _,
    } = config;
    let boundary = outcome
        .iter()
        .enumerate()
        .filter_map(|(row, value)| (*value == 0.0).then_some(row))
        .collect::<Vec<_>>();
    if boundary.is_empty() {
        return Ok(DetectionProposal::empty(
            SeparationMethod::IteratedRectifier,
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
    if !certificate_tolerance.is_finite()
        || certificate_tolerance <= 0.0
        || certificate_tolerance >= 1.0
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "iterated-rectifier certificate tolerance must lie strictly between zero and one",
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
        let positive_direction = projected.fitted;
        let direction_scale = positive_direction
            .iter()
            .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
        if !direction_scale.is_finite() {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "iterated-rectifier direction scale became nonfinite",
            ));
        }
        if direction_scale <= f64::MIN_POSITIVE {
            return Ok(zero_result(
                iteration,
                0.0,
                "iterated rectifier converged to the exact zero direction",
            ));
        }

        // `u` is normalized to at most one.  The fixed unit floor is used only
        // for deciding convergence of the rectifier recurrence.  Any actual
        // deletion direction is certified below on its own infinity scale.
        let recurrence_scale = direction_scale.max(1.0);
        let recurrence_threshold = tolerance * recurrence_scale;
        let interior_absolute = outcome
            .iter()
            .zip(&positive_direction)
            .filter(|(outcome, _)| **outcome > 0.0)
            .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()));
        let minimum_boundary = boundary
            .iter()
            .map(|row| positive_direction[*row])
            .fold(f64::INFINITY, f64::min);
        let maximum_boundary = boundary
            .iter()
            .map(|row| positive_direction[*row])
            .fold(f64::NEG_INFINITY, f64::max);

        if minimum_boundary >= -recurrence_threshold
            && maximum_boundary <= recurrence_threshold
            && interior_absolute <= certificate_tolerance * recurrence_scale
            && (-minimum_boundary).max(0.0) <= certificate_tolerance * recurrence_scale
        {
            return Ok(DetectionProposal {
                method: SeparationMethod::IteratedRectifier,
                dropped: Vec::new(),
                iterations: iteration,
                status: MethodStatus::NoSeparation,
                certificate: Some(SeparationCertificate {
                    proof: crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint,
                    interior_zero_relative: interior_absolute / recurrence_scale,
                    boundary_violation_relative: (-minimum_boundary).max(0.0) / recurrence_scale,
                    span_residual_relative: 0.0,
                    negative_margin_relative: 0.0,
                    direction_scale,
                    positive_maximum: -minimum_boundary,
                    passed: true,
                }),
                exact_fe_certificate: None,
                direction: None,
                note: "iterated rectifier converged to the zero certificate".to_owned(),
            });
        }

        if minimum_boundary >= -recurrence_threshold && maximum_boundary > recurrence_threshold {
            let direction = positive_direction
                .iter()
                .map(|value| -*value)
                .collect::<Vec<_>>();
            let proposal_threshold = tolerance * direction_scale;
            let dropped = boundary
                .iter()
                .copied()
                .filter(|row| direction[*row] < -proposal_threshold)
                .collect::<Vec<_>>();
            let span_check =
                project_onto_model(&direction, regressors, &weights, fixed_effects, projection)?;
            let span_denominator = weighted_norm(&direction, &weights)?.max(f64::MIN_POSITIVE);
            let span_residual_relative =
                weighted_norm(&span_check.residual, &weights)? / span_denominator;
            let interior_zero_relative = interior_absolute / direction_scale;
            let boundary_violation_relative = direction
                .iter()
                .enumerate()
                .filter(|(row, _)| outcome[*row] == 0.0)
                .map(|(_, value)| *value)
                .fold(f64::NEG_INFINITY, f64::max)
                .max(0.0)
                / direction_scale;
            let negative_margin_relative = dropped
                .iter()
                .map(|row| -direction[*row] / direction_scale)
                .fold(f64::INFINITY, f64::min);
            let passed = !dropped.is_empty()
                && span_check.certificate.passed
                && span_residual_relative <= certificate_tolerance
                && interior_zero_relative <= certificate_tolerance
                && boundary_violation_relative <= certificate_tolerance
                && negative_margin_relative >= certificate_tolerance;
            if passed {
                return Ok(DetectionProposal {
                    method: SeparationMethod::IteratedRectifier,
                    dropped,
                    iterations: iteration,
                    status: MethodStatus::Certified,
                    certificate: Some(SeparationCertificate {
                        proof:
                            crate::ppml_separation::SeparationCertificateProof::RecessionDirection,
                        interior_zero_relative,
                        boundary_violation_relative,
                        span_residual_relative,
                        negative_margin_relative,
                        direction_scale,
                        positive_maximum: direction
                            .iter()
                            .enumerate()
                            .filter(|(row, _)| outcome[*row] == 0.0)
                            .map(|(_, value)| *value)
                            .fold(f64::NEG_INFINITY, f64::max),
                        passed: true,
                    }),
                    exact_fe_certificate: None,
                    direction: Some(direction),
                    note: "iterated-rectifier separation certificate passed".to_owned(),
                });
            }
        }

        // An apparent direction that does not yet certify is another
        // recurrence state, never deletion evidence.  Continue until either a
        // certificate passes or the registered budget is exhausted.
        let mut maximum = 0.0_f64;
        for row in 0..u.len() {
            u[row] = if outcome[row] > 0.0 || positive_direction[row].abs() <= recurrence_threshold
            {
                0.0
            } else {
                positive_direction[row].max(0.0)
            };
            maximum = maximum.max(u[row]);
        }
        if maximum > 0.0 {
            u.iter_mut().for_each(|value| *value /= maximum);
        }
    }

    Ok(DetectionProposal {
        method: SeparationMethod::IteratedRectifier,
        dropped: Vec::new(),
        iterations: max_iterations,
        status: MethodStatus::MaximumIterations,
        certificate: None,
        exact_fe_certificate: None,
        direction: None,
        note: format!("iterated-rectifier separation check exhausted {max_iterations} iterations"),
    })
}

fn zero_result(iteration: usize, direction_scale: f64, note: &str) -> DetectionProposal {
    DetectionProposal {
        method: SeparationMethod::IteratedRectifier,
        dropped: Vec::new(),
        iterations: iteration,
        status: MethodStatus::NoSeparation,
        certificate: Some(SeparationCertificate {
            proof: crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint,
            interior_zero_relative: 0.0,
            boundary_violation_relative: 0.0,
            span_residual_relative: 0.0,
            negative_margin_relative: 0.0,
            direction_scale,
            positive_maximum: 0.0,
            passed: true,
        }),
        exact_fe_certificate: None,
        direction: None,
        note: note.to_owned(),
    }
}
