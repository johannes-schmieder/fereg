use super::*;
pub(super) fn support(
    model: &PreparedPpmlModelGeneration,
    method: SeparationMethod,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<PreparedNumericalProposal>> {
    let topology = DetectorTopology::new(model.prepared())?;
    let count = model.outcome().iter().filter(|&&y| y > 0.).count();
    let meta =
        topology.sources.len() + topology.effect_specs.len() + topology.effect_to_source.len();
    let marker = match method {
        SeparationMethod::IteratedRectifier => PpmlInterruptPoint::SeparationIrIteration,
        SeparationMethod::Mu => PpmlInterruptPoint::SeparationMu,
        _ => PpmlInterruptPoint::SeparationSimplex,
    };
    let limit = if method == SeparationMethod::Simplex {
        config.simplex_max_tableau_cells
    } else {
        primal::workspace(model, config)
    };
    positive_rank::detect_for(
        method,
        model,
        &topology,
        count,
        limit.saturating_sub(meta),
        config,
        &mut |_| interrupt(marker),
    )?
    .map(|raw| bind_numerical_proposal(model, raw))
    .transpose()
}

pub(super) fn unresolved() -> FeregError {
    FeregError::new(ErrorCode::NonConvergence,"could not verify an exact full-model recession direction for the proposed separated observations")
}

pub(super) fn candidate(
    model: &PreparedPpmlModelGeneration,
    proposal: PreparedNumericalProposal,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedNumericalProposal> {
    let method = proposal.method;
    match checked_candidate(model, proposal, config, interrupt) {
        Err(error)
            if method == SeparationMethod::Simplex
                && config.methods.iterated_rectifier
                && error == unresolved() =>
        {
            bind_numerical_proposal(
                model,
                DetectionProposal::empty(
                    method,
                    MethodStatus::Uncertified,
                    "model simplex deletion lacks an exact recession proof",
                ),
            )
        }
        result => result,
    }
}

fn checked_candidate(
    model: &PreparedPpmlModelGeneration,
    mut proposal: PreparedNumericalProposal,
    config: SeparationConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<PreparedNumericalProposal> {
    if proposal.dropped_original_rows.is_empty() || proposal.exact_intercept_span.is_some() {
        return Ok(proposal);
    }
    let (detector_direction, _) = checked_deletion_claim(model, &proposal, config)?;
    let (proof, complete) = match primal::find(
        model,
        &proposal.dropped_original_rows,
        config,
        detector_direction,
        true,
    )? {
        Some(proved) => proved,
        None => {
            if let Some(no_drop) = support(model, proposal.method, config, interrupt)? {
                return Ok(no_drop);
            }
            // A spurious near-separator may point exclusively to impossible
            // rows while a different recession ray exists elsewhere. Release
            // its guessed binding zero rows and search the full boundary.
            let zeros = fallible_collect(
                model
                    .original_rows()
                    .iter()
                    .zip(model.outcome())
                    .filter_map(|(&r, &y)| (y == 0.).then_some(r)),
                model.outcome().len(),
                "unable to reserve exact boundary search rows",
            )?;
            if zeros == proposal.dropped_original_rows {
                return Err(unresolved());
            }
            let (other, _) = primal::find(model, &zeros, config, detector_direction, false)?
                .ok_or_else(unresolved)?;
            (other, false)
        }
    };
    // A stored coefficient ray permits direct span replay even when a
    // floating re-projection of this same ill-conditioned model fails.
    if let Some(raw) = materialize(model, &proof, proposal.method, config)? {
        let rows = compact_to_original_exact_capacity(raw.dropped, model.original_rows())?;
        if !complete || rows == proposal.dropped_original_rows {
            proposal.dropped_original_rows = rows;
            proposal.certificate = raw.certificate;
            proposal.direction = raw.direction;
        }
    } else if !complete {
        return Err(unresolved());
    }
    proposal.exact_primal = Some(proof);
    Ok(proposal)
}

fn materialize(
    model: &PreparedPpmlModelGeneration,
    proof: &primal::Witness,
    method: SeparationMethod,
    config: SeparationConfig,
) -> Result<Option<DetectionProposal>> {
    let Some(direction) = primal::direction(model, proof)? else {
        return Ok(None);
    };
    let scale = infinity_norm(&direction);
    let dropped = fallible_collect(
        direction.iter().enumerate().filter_map(|(r, &z)| {
            (model.outcome()[r] == 0. && z < -config.tolerance * scale).then_some(r)
        }),
        direction.len(),
        "unable to reserve repaired strict rows",
    )?;
    if dropped.is_empty() {
        return Ok(None);
    }
    let certificate = detector_direction_certificate(
        model.outcome(),
        &direction,
        &dropped,
        config.certificate_tolerance,
    )?;
    Ok(Some(DetectionProposal {
        method,
        dropped,
        iterations: 1,
        status: MethodStatus::Certified,
        certificate: Some(certificate),
        exact_fe_certificate: None,
        direction: Some(direction),
        note: "exact raw-design recession direction".into(),
    }))
}

/// A failed numerical iteration may never have emitted a usable search
/// direction. Search all zero outcomes and return only a materialized ray.
pub(super) fn raw_candidate(
    model: &PreparedPpmlModelGeneration,
    method: SeparationMethod,
    config: SeparationConfig,
) -> Result<Option<DetectionProposal>> {
    let zeros = fallible_collect(
        model
            .original_rows()
            .iter()
            .zip(model.outcome())
            .filter_map(|(&r, &y)| (y == 0.).then_some(r)),
        model.outcome().len(),
        "unable to reserve exact boundary search rows",
    )?;
    if zeros.is_empty() {
        return Ok(None);
    }
    let hint = fallible_collect(
        model
            .outcome()
            .iter()
            .map(|&y| if y == 0. { -1. } else { 0. }),
        model.outcome().len(),
        "unable to reserve boundary search targets",
    )?;
    let Some((proof, _)) = primal::find(model, &zeros, config, &hint, false)? else {
        return Ok(None);
    };
    materialize(model, &proof, method, config)
}
