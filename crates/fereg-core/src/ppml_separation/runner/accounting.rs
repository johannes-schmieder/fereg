use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;
use crate::ols::FiniteSolver;
use crate::ppml::PpmlConfig;
use crate::ppml_separation::fe::propose_zero_only_fixed_effect_levels;
use crate::ppml_separation::projection::{
    project_onto_model, weighted_norm, ProjectionConfig, LEGACY_PROJECTION_RANK_TOLERANCE,
};
use crate::ppml_separation::subset::SubsetProblem;
use crate::ppml_separation::{
    DetectionProposal, MethodStatus, SeparationCertificate, SeparationConfig, SeparationMethod,
};

pub(super) fn validate_detection_inputs(
    outcome: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    config: SeparationConfig,
) -> Result<()> {
    if outcome.len() != regressors.nrows()
        || fixed_effects.is_some_and(|design| design.nobs() != outcome.len())
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "separation inputs have inconsistent observation counts",
        ));
    }
    if outcome
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        || regressors.as_slice().iter().any(|value| !value.is_finite())
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "separation requires finite nonnegative outcomes and finite regressors",
        ));
    }
    for value in [
        config.tolerance,
        config.certificate_tolerance,
        config.projection_tolerance,
        config.mu_tolerance,
    ] {
        if !value.is_finite() || value <= 0.0 || value >= 1.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "separation tolerances must lie strictly between zero and one",
            ));
        }
    }
    if config.max_ir_iterations == 0
        || config.simplex_max_iterations == 0
        || config.simplex_max_tableau_cells == 0
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "separation iteration and memory budgets must be positive",
        ));
    }
    Ok(())
}

pub(super) fn map_proposal_to_original(
    mut proposal: DetectionProposal,
    original_rows: &[usize],
) -> Result<DetectionProposal> {
    proposal.dropped = proposal
        .dropped
        .into_iter()
        .map(|row| {
            original_rows.get(row).copied().ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "separation detector returned an out-of-range compact observation",
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(proposal)
}

/// Validate a detector output without changing the production ledger.
/// Numerical proposals are independently reprojected and recertified here.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_proposal(
    proposal: &DetectionProposal,
    expected_method: SeparationMethod,
    outcome: &[f64],
    fixed_effects: Option<&FeDesign>,
    active: &[bool],
    subset: Option<&SubsetProblem>,
    ppml: PpmlConfig,
    config: SeparationConfig,
) -> Result<Option<SeparationCertificate>> {
    if proposal.method != expected_method {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "separation detector returned the wrong method identity",
        ));
    }
    validate_status_shape(proposal)?;
    if proposal.dropped.windows(2).any(|rows| rows[0] >= rows[1]) {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "separation proposal rows must be sorted and unique",
        ));
    }
    for &row in &proposal.dropped {
        if row >= active.len() {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "separation proposal contains an out-of-range original observation",
            ));
        }
        if !active[row] {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "separation proposal contains an inactive original observation",
            ));
        }
        if outcome[row] != 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "separation proposal attempted to delete a positive-outcome observation",
            ));
        }
    }

    match expected_method {
        SeparationMethod::FixedEffects => {
            if proposal.direction.is_some() || proposal.certificate.is_some() {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "exact-FE proposal supplied numerical evidence",
                ));
            }
            let recomputed = propose_zero_only_fixed_effect_levels(outcome, fixed_effects, active);
            if recomputed.dropped != proposal.dropped
                || recomputed.exact_fe_certificate != proposal.exact_fe_certificate
            {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "exact-FE proposal failed its independent combinatorial certificate",
                ));
            }
            Ok(None)
        }
        SeparationMethod::Simplex | SeparationMethod::IteratedRectifier | SeparationMethod::Mu
            if !proposal.dropped.is_empty() =>
        {
            let detector_certificate = proposal.certificate.as_ref().ok_or_else(|| {
                FeregError::new(
                    ErrorCode::NonConvergence,
                    "numerical separation proposal omitted its detector certificate",
                )
            })?;
            if !detector_certificate.passed {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "numerical separation detector reported an uncertified proposal",
                ));
            }
            let subset = subset.ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Internal,
                    "numerical separation proposal lacks its immutable prepared view",
                )
            })?;
            let direction = proposal.direction.as_deref().ok_or_else(|| {
                FeregError::new(
                    ErrorCode::NonConvergence,
                    "numerical separation proposal omitted its recession direction",
                )
            })?;
            let certificate = independently_certify_direction(
                subset,
                direction,
                &proposal.dropped,
                ppml,
                config,
            )?;
            if !certificate.passed {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    format!(
                        "{} proposal failed coordinator certification: interior={:.3e}, boundary={:.3e}, span={:.3e}, margin={:.3e}",
                        method_label(expected_method),
                        certificate.interior_zero_relative,
                        certificate.boundary_violation_relative,
                        certificate.span_residual_relative,
                        certificate.negative_margin_relative,
                    ),
                ));
            }
            Ok(Some(certificate))
        }
        SeparationMethod::Singletons => Err(FeregError::new(
            ErrorCode::Internal,
            "singleton pruning is a ledger operation, not a detector proposal",
        )),
        _ => {
            if proposal.direction.is_some() && proposal.status != MethodStatus::NoSeparation {
                return Err(FeregError::new(
                    ErrorCode::Internal,
                    "noncertified detector status retained a numerical direction",
                ));
            }
            if matches!(
                expected_method,
                SeparationMethod::IteratedRectifier | SeparationMethod::Mu
            ) && proposal.status == MethodStatus::NoSeparation
            {
                let certificate = proposal.certificate.as_ref().ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::NonConvergence,
                        "comprehensive no-separation result omitted its certificate",
                    )
                })?;
                if !certificate.passed || !certificate_is_finite(certificate) {
                    return Err(FeregError::new(
                        ErrorCode::NonConvergence,
                        "comprehensive no-separation certificate is invalid",
                    ));
                }
            }
            Ok(proposal.certificate.clone())
        }
    }
}

fn validate_status_shape(proposal: &DetectionProposal) -> Result<()> {
    match proposal.status {
        MethodStatus::Certified if proposal.dropped.is_empty() => Err(FeregError::new(
            ErrorCode::Internal,
            "certified separation proposal contains no rows",
        )),
        MethodStatus::NoCandidates
        | MethodStatus::NoSeparation
        | MethodStatus::SkippedMemory
        | MethodStatus::SkippedUnsupported
        | MethodStatus::MaximumIterations
        | MethodStatus::Uncertified
            if !proposal.dropped.is_empty() =>
        {
            Err(FeregError::new(
                ErrorCode::Internal,
                "noncertified separation status contains proposed rows",
            ))
        }
        MethodStatus::SkippedMemory if proposal.method != SeparationMethod::Simplex => {
            Err(FeregError::new(
                ErrorCode::Internal,
                "only the bounded simplex detector may report a memory skip",
            ))
        }
        _ => Ok(()),
    }
}

fn independently_certify_direction(
    subset: &SubsetProblem,
    direction: &[f64],
    proposed_original_rows: &[usize],
    ppml: PpmlConfig,
    config: SeparationConfig,
) -> Result<SeparationCertificate> {
    if direction.len() != subset.outcome.len() || direction.iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "separation direction has invalid dimensions or nonfinite values",
        ));
    }
    let direction_scale = direction
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    if !direction_scale.is_finite() || direction_scale <= f64::MIN_POSITIVE {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "separation direction has zero or nonfinite scale",
        ));
    }
    let detection_threshold = config.tolerance * direction_scale;
    let expected_rows = subset
        .outcome
        .iter()
        .zip(direction)
        .zip(&subset.original_rows)
        .filter_map(|((outcome, direction), original)| {
            (*outcome == 0.0 && *direction < -detection_threshold).then_some(*original)
        })
        .collect::<Vec<_>>();
    if expected_rows != proposed_original_rows {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "separation proposal is not the exact strict-negative set of its direction",
        ));
    }

    let interior_zero_relative = subset
        .outcome
        .iter()
        .zip(direction)
        .filter(|(outcome, _)| **outcome > 0.0)
        .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()))
        / direction_scale;
    let positive_maximum = subset
        .outcome
        .iter()
        .zip(direction)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| *value)
        .fold(f64::NEG_INFINITY, f64::max);
    let boundary_violation_relative = positive_maximum.max(0.0) / direction_scale;
    let negative_margin_relative = proposed_original_rows
        .iter()
        .map(|original| {
            subset
                .original_rows
                .binary_search(original)
                .map(|row| -direction[row] / direction_scale)
                .map_err(|_| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "proposed original row is absent from its detector view",
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .fold(f64::INFINITY, f64::min);

    let weights = vec![1.0_f64; direction.len()];
    let projection = project_onto_model(
        direction,
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
    let span_residual_relative = weighted_norm(&projection.residual, &weights)?
        / weighted_norm(direction, &weights)?.max(f64::MIN_POSITIVE);
    let passed = projection.certificate.passed
        && interior_zero_relative <= config.certificate_tolerance
        && boundary_violation_relative <= config.certificate_tolerance
        && span_residual_relative <= config.certificate_tolerance
        && negative_margin_relative >= config.certificate_tolerance;
    Ok(SeparationCertificate {
        proof: crate::ppml_separation::SeparationCertificateProof::RecessionDirection,
        interior_zero_relative,
        boundary_violation_relative,
        span_residual_relative,
        negative_margin_relative,
        direction_scale,
        positive_maximum,
        passed,
    })
}

fn certificate_is_finite(certificate: &SeparationCertificate) -> bool {
    certificate.interior_zero_relative.is_finite()
        && certificate.boundary_violation_relative.is_finite()
        && certificate.span_residual_relative.is_finite()
        && certificate.negative_margin_relative.is_finite()
        && certificate.direction_scale.is_finite()
        && certificate.positive_maximum.is_finite()
}

fn method_label(method: SeparationMethod) -> &'static str {
    match method {
        SeparationMethod::Singletons => "singleton",
        SeparationMethod::FixedEffects => "exact-FE",
        SeparationMethod::Simplex => "simplex",
        SeparationMethod::IteratedRectifier => "iterated-rectifier",
        SeparationMethod::Mu => "mu",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_rows_are_rejected_before_any_commit() {
        let outcome = [0.0, 1.0];
        let regressors = DenseMatrix::from_columns(&[vec![1.0, 0.0]]).expect("matrix");
        let subset = crate::ppml_separation::subset::subset_problem(
            &outcome,
            &regressors,
            None,
            &[true, true],
        )
        .expect("subset");
        let proposal = DetectionProposal {
            method: SeparationMethod::Simplex,
            dropped: vec![0, 0],
            iterations: 1,
            status: MethodStatus::Certified,
            certificate: Some(SeparationCertificate {
                proof: crate::ppml_separation::SeparationCertificateProof::RecessionDirection,
                interior_zero_relative: 0.0,
                boundary_violation_relative: 0.0,
                span_residual_relative: 0.0,
                negative_margin_relative: 1.0,
                direction_scale: 1.0,
                positive_maximum: -1.0,
                passed: true,
            }),
            exact_fe_certificate: None,
            direction: Some(vec![-1.0, 0.0]),
            note: "".into(),
        };
        let error = validate_proposal(
            &proposal,
            SeparationMethod::Simplex,
            &outcome,
            None,
            &[true, true],
            Some(&subset),
            PpmlConfig::default(),
            SeparationConfig::default(),
        )
        .expect_err("duplicate proposal");
        assert_eq!(error.code, ErrorCode::Internal);
    }
}
