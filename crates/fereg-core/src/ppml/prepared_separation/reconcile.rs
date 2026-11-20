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
    let limit = primal::ProofBudget::for_sparse(model, method, config)
        .words
        .saturating_sub(meta);
    let mut report_break = |_| interrupt(marker);
    let mut result = positive_rank::detect_for(
        method,
        model,
        &topology,
        count,
        (limit, positive_rank::sparse_dual::Search::Full),
        config,
        &mut report_break,
    )?;
    if result.is_none() {
        result = positive_rank::aliased(
            method,
            model,
            &topology,
            count,
            limit,
            config,
            &mut report_break,
        )?;
    }
    if result.is_none() {
        result = positive_rank::exact_dual(method, model, &topology, count, limit, config)?;
    }
    result
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
    if proposal.dropped_original_rows.is_empty()
        || proposal.exact_graph_span.is_some()
        || proposal.exact_primal.is_some()
    {
        return Ok(proposal);
    }
    let (detector_direction, _) = checked_deletion_claim(model, &proposal, config)?;
    let budget = primal::ProofBudget::new(model, proposal.method, config);
    let (proof, complete) = match primal::find(
        model,
        &proposal.dropped_original_rows,
        budget,
        detector_direction,
        true,
    )? {
        Some(proved) => proved,
        None => {
            if let Some(indicators) = factor_pair(model, proposal.method, config)? {
                return Ok(indicators);
            }
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
            let (other, _) = primal::find(model, &zeros, budget, detector_direction, false)?
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

pub(super) fn omitted_factor(
    model: &PreparedPpmlModelGeneration,
    method: SeparationMethod,
    config: SeparationConfig,
) -> Result<Option<PreparedNumericalProposal>> {
    let budget = primal::ProofBudget::for_sparse(model, method, config);
    let Some(witness) = primal::omitted_factor(model, budget)? else {
        return Ok(None);
    };
    bind_witness(model, method, config, witness)
}

pub(super) fn sparse_slope_ray(
    model: &PreparedPpmlModelGeneration,
    method: SeparationMethod,
    config: SeparationConfig,
) -> Result<Option<PreparedNumericalProposal>> {
    let budget = primal::ProofBudget::for_sparse(model, method, config);
    let Some(witness) = primal::sparse_interpolant(model, budget)? else {
        return Ok(None);
    };
    let proposal = bind_witness(model, method, config, witness)?;
    if crate::diagnostics::solver_trace_enabled() {
        if let Some(p) = &proposal {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=separation_sparse_primal|generation={}|method={method:?}|rows={}|proposed={}",
                model.generation(), model.outcome().len(), p.dropped_original_rows.len()
            ));
        }
    }
    Ok(proposal)
}

pub(super) fn factor_pair(
    model: &PreparedPpmlModelGeneration,
    method: SeparationMethod,
    config: SeparationConfig,
) -> Result<Option<PreparedNumericalProposal>> {
    let budget = primal::ProofBudget::for_sparse(model, method, config);
    let Some(witness) = primal::factor_pair(model, budget)? else {
        return Ok(None);
    };
    let proposal = bind_witness(model, method, config, witness)?;
    if crate::diagnostics::solver_trace_enabled() {
        if let Some(p) = &proposal {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=separation_factor_pair|generation={}|method={method:?}|rows={}|proposed={}",
                model.generation(), model.outcome().len(), p.dropped_original_rows.len()
            ));
        }
    }
    Ok(proposal)
}

fn bind_witness(
    model: &PreparedPpmlModelGeneration,
    method: SeparationMethod,
    config: SeparationConfig,
    witness: primal::Witness,
) -> Result<Option<PreparedNumericalProposal>> {
    let Some(mut raw) = materialize(model, &witness, method, config)? else {
        return Ok(None);
    };
    raw.iterations = 0;
    let mut bound = bind_numerical_proposal(model, raw)?;
    bound.exact_primal = Some(witness);
    Ok(Some(bound))
}

pub(super) fn compact_ray(
    model: &PreparedPpmlModelGeneration,
    mut coefficients: Vec<i64>,
    iterations: usize,
    method: SeparationMethod,
    config: SeparationConfig,
) -> Result<Option<PreparedNumericalProposal>> {
    let finite = model.model().columns.len();
    let available = if method == SeparationMethod::Simplex {
        config.simplex_max_tableau_cells
    } else {
        primal::ProofBudget::for_sparse(model, method, config).words
    };
    if finite > coefficients.len()
        || primal::RawRows::sparse_replay_words(model, coefficients.capacity()) > available
    {
        return Ok(None);
    }
    // The simplex has finite columns first. The sparse witness visits each
    // row's absorbed terms followed by the same finite columns. Its exact
    // dyadic accumulator omits implicit zeros and rounds each score once.
    coefficients.rotate_left(finite);
    let mut proposal = bind_witness(model, method, config, primal::Witness::Sparse(coefficients))?;
    if let Some(proof) = &mut proposal {
        proof.iterations = iterations;
    }
    Ok(proposal)
}

/// A capped simplex returned only coefficients. Their first original-row
/// materialization and independent acceptance belong to this IR phase.
pub(super) fn certify_compact_hint(
    model: &PreparedPpmlModelGeneration,
    coefficients: Vec<i64>,
    config: SeparationConfig,
    policy: &retry::DetectorPolicy<'_>,
) -> Result<Option<ValidatedNumericalProposal>> {
    let proof = compact_ray(
        model,
        coefficients,
        0,
        SeparationMethod::IteratedRectifier,
        config,
    )
    .and_then(|proposal| {
        proposal
            .map(|p| policy.validate(model, p, config))
            .transpose()
    });
    match proof {
        Err(error) if error.code == ErrorCode::NonConvergence => Ok(None),
        other => other,
    }
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
        note: crate::ppml_separation::EXACT_RAW_RAY_NOTE.into(),
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
    let budget = primal::ProofBudget::new(model, method, config);
    let Some((proof, _)) = primal::find(model, &zeros, budget, &hint, false)? else {
        return Ok(None);
    };
    materialize(model, &proof, method, config)
}
