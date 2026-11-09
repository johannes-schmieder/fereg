//! Saved-effect recovery on fedemean's full formulation, and fereg's
//! representative of singular slope levels.
//!
//! fedemean returns a minimum-norm representative (in scaled coordinates) on
//! a level whose channels are dependent on its rows; fereg reports the
//! representative of its scaled pivoted group solve (the intercept kept on
//! ties, dropped pivots exactly zero). Both have the same fitted
//! contribution, so each such level is re-expressed by that solve on its own
//! fitted values. Other levels keep their coefficients bitwise.

use super::*;

/// Re-express, on every level of a slope-bearing effect whose channels the
/// scaled pivoted group solve finds rank deficient, the level's fitted
/// contribution in that solve's representative. `coefficients` are one
/// channel-major column (`effect_offset + channel * levels + level`).
/// Returns the number of re-expressed levels.
pub(crate) fn canonicalize_singular_slope_levels(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: ValidatedWeights<'_>,
    coefficients: &mut [f64],
) -> Result<usize> {
    let offsets = channel_major_effect_offsets(topology, effects)?;
    if coefficients.len() != offsets.last().copied().unwrap_or(0) {
        return Err(invalid_argument(
            "singular slope level coefficients do not match the effects",
        ));
    }
    let Some(max_channels) = effects
        .iter()
        .filter(|effect| !effect.slopes.is_empty())
        .map(CategoricalAbsorbedEffectView::channels)
        .max()
    else {
        return Ok(0);
    };
    let mut workspace = SlopeWorkspace::new(max_channels, 1, &mut SystemSlopeWorkspaceReservation)?;
    let mut fitted = DenseMatrix::from_row_major(
        topology.nobs(),
        1,
        fallible_filled(
            topology.nobs(),
            0.0,
            "unable to reserve singular slope level contributions",
        )?,
    )?;
    let mut changed = 0_usize;
    for (index, effect) in effects.iter().enumerate() {
        if effect.slopes.is_empty() {
            continue;
        }
        let source = topology
            .registry()
            .source(topology.effect_to_topology()[index])?;
        let (levels, channels) = (source.n_levels(), effect.channels());
        let at = |channel: usize, level: usize| offsets[index] + channel * levels + level;
        for level in 0..levels {
            let members = source.members(level)?;
            for &row in members {
                let mut value = 0.0_f64;
                for channel in 0..channels {
                    let basis = if effect.intercept && channel == 0 {
                        1.0
                    } else {
                        effect.slopes[channel - usize::from(effect.intercept)][row]
                    };
                    value = coefficients[at(channel, level)].mul_add(basis, value);
                }
                fitted[(row, 0)] = value;
            }
            group_moments(members, effect, weights, &fitted, &mut workspace)?;
            if solve_group(channels, 1, &mut workspace)? == channels {
                continue;
            }
            for channel in 0..channels {
                let value = workspace.coefficients[channel] / workspace.channel_scales[channel];
                if !value.is_finite() {
                    return Err(invalid_data(
                        "singular slope level representative is not finite",
                    ));
                }
                coefficients[at(channel, level)] = value;
            }
            changed += 1;
        }
    }
    Ok(changed)
}

/// The inputs of [`recover_finalized_full_certified`] beyond the target.
pub(crate) struct FullRecovery<'a> {
    /// A coefficient column to start from (the fit's retained product).
    pub(crate) start: Option<&'a FeCoefficientBatch>,
    pub(crate) config: MapConfig,
    /// fedemean declines a larger solver workspace.
    pub(crate) memory_limit: Option<u128>,
    /// Strict PPML recovery: every row's reconstruction from the normalized
    /// coefficients within this relative tolerance.
    pub(crate) reconstruction_tolerance: Option<f64>,
    pub(crate) known_connected: bool,
    pub(crate) materialize_absorbed_sum: bool,
}

/// Recover saved effects with fedemean's full system (PCG with the levels'
/// channel blocks, escalating to the hybrid Schwarz), from the retained
/// coefficients or zero, at `config.tolerance` within `config.max_iterations`
/// and requesting the coefficients. They are re-expressed in fereg's
/// representative on singular slope levels, normalized per SPEC §5, and the
/// fresh reconstruction of the target and the independent FE certificate
/// decide (as for retained coefficients). A decline is a nonconvergence (the
/// caller's MAP recovery follows). Returns the recovery and fedemean's
/// workspace bytes.
pub(crate) fn recover_finalized_full_certified(
    topology: &FinalizedCategoricalTopology,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    weights: BorrowedMapWeights<'_>,
    (target, requested): (&[f64], &[bool]),
    options: FullRecovery<'_>,
) -> Result<(RecoveredCategoricalEffects, u128)> {
    validate_recovery_inputs(topology, effects, target, requested)?;
    validate_config(options.config)?;
    let validated = validate_weights(weights, target.len())?;
    let offsets = channel_major_effect_offsets(topology, effects)?;
    let sources = topology_source_table(topology)?;
    let borrowed = borrow_categorical_topology_design(
        target.len(),
        topology.registry(),
        &sources,
        topology.effect_to_topology(),
    )?;
    let per_rhs = offsets.last().copied().unwrap_or(0);
    // The coefficient and target copies share the limit with fedemean's
    // workspace; a limit below them leaves the caller's MAP recovery.
    let copies = 8 * (per_rhs as u128 + target.len() as u128);
    let memory_limit = match options.memory_limit {
        Some(limit) if limit < copies => {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "saved-FE full-system recovery does not fit the memory limit",
            ))
        }
        limit => limit.map(|limit| limit - copies),
    };
    if let Some(start) = options.start {
        validate_coefficient_layout(topology, effects, start)?;
    }
    let mut coefficients = fallible_filled(
        per_rhs,
        0.0,
        "unable to reserve full-system recovery coefficients",
    )?;
    let (iterations, workspace) = project_target(
        (&borrowed, effects, &offsets),
        weights,
        target,
        (options.start, &mut coefficients),
        (options.config, memory_limit),
    )?;
    let singular =
        canonicalize_singular_slope_levels(topology, effects, validated, &mut coefficients)?;
    normalize_intercept_components(
        topology,
        effects,
        validated,
        &mut coefficients,
        options.known_connected,
    )?;
    if let Some(tolerance) = options.reconstruction_tolerance {
        if !channel_major_reconstruction_passed(
            &borrowed,
            effects,
            target,
            &offsets,
            &coefficients,
            tolerance,
        ) {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "full-system recovery coefficient product failed original-target reconstruction",
            ));
        }
    }
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=saved_slope_recovery|method=fedemean_full|iterations={iterations}|warm_start={}|singular_levels={singular}",
            u8::from(options.start.is_some())
        ));
    }
    let recovered = finish_channel_major_recovery(
        topology,
        effects,
        validated,
        target,
        requested,
        &coefficients,
        iterations,
        options.config.tolerance,
        CategoricalRecoveryMethod::Full,
        options.materialize_absorbed_sum,
    )?;
    Ok((recovered, workspace.saturating_add(copies)))
}

/// Project `target` with fedemean's full system (PCG with the levels'
/// channel blocks, escalating to the hybrid Schwarz) from `start` or zero,
/// adding the fitted coefficients; a decline is a nonconvergence. Returns the
/// iterations and fedemean's workspace bytes.
fn project_target(
    (borrowed, effects, offsets): (
        &crate::fe::BorrowedInterceptFeDesign<'_>,
        &[CategoricalAbsorbedEffectView<'_>],
        &[usize],
    ),
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    (start, coefficients): (Option<&FeCoefficientBatch>, &mut [f64]),
    (config, memory_limit): (MapConfig, Option<u128>),
) -> Result<(usize, u128)> {
    let fedemean_error = crate::fe::block_schur::fedemean_error;
    let mut view =
        fallible_with_capacity(effects.len(), "unable to reserve recovery effect views")?;
    let mut levels = fallible_with_capacity(effects.len(), "unable to reserve recovery levels")?;
    let mut channels =
        fallible_with_capacity(effects.len(), "unable to reserve recovery channels")?;
    for (effect, entry) in effects.iter().zip(borrowed.effects()) {
        view.push(
            fedemean::Effect::new(entry.codes(), entry.n_levels())
                .with_slopes(effect.intercept, effect.slopes.iter().copied())
                .map_err(fedemean_error)?,
        );
        levels.push(entry.n_levels());
        channels.push(effect.channels());
    }
    let design = fedemean::Design::new(target.len(), view).map_err(fedemean_error)?;
    let mut projector = fedemean::Projector::new(
        design,
        fedemean::Settings {
            choice: fedemean::Choice {
                reduction: fedemean::Reduction::Full,
                method: fedemean::Method::Pcg,
                preconditioner: fedemean::Preconditioner::Auto,
                fallback: fedemean::Fallback::Escalate,
            },
            ..fedemean::Settings::default()
        },
    );
    let per_rhs = offsets.last().copied().unwrap_or(0);
    let mut residual = fallible_copy(target, "unable to reserve full-system recovery target")?;
    let layout = fedemean::CoefficientLayout::new(per_rhs, &offsets[..effects.len()], &levels)
        .with_channels(&channels);
    let mut request = fedemean::Request::new(config.tolerance, config.max_iterations);
    request.memory_limit = memory_limit;
    let fe_weights = match weights {
        BorrowedMapWeights::Unit => fedemean::Weights::Unit,
        BorrowedMapWeights::Provided(values) => fedemean::Weights::Values(values),
    };
    let control = fedemean::Control::with_cancel(&crate::interrupt::cancellation_requested);
    let columns =
        fedemean::ColumnsMut::new(&mut residual, target.len(), 1).map_err(fedemean_error)?;
    let output = Some(fedemean::CoefficientsMut::new(coefficients, layout));
    let outcome = match start {
        Some(start) => projector.project_from(
            fe_weights,
            columns,
            start.rhs(0)?,
            output,
            &request,
            &control,
        ),
        None => projector.project(fe_weights, columns, output, &request, &control),
    }
    .map_err(fedemean_error)?;
    match outcome {
        fedemean::Outcome::Projected(report) => {
            Ok((report.facts.iterations, report.workspace_bytes))
        }
        fedemean::Outcome::Declined { reason, facts, .. } => Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!(
                "saved-FE full-system recovery declined after {} iterations ({})",
                facts.iterations,
                crate::fe::fedemean_slope::describe(reason)
            ),
        )),
        _ => Err(FeregError::new(
            ErrorCode::NonConvergence,
            "saved-FE full-system recovery did not project",
        )),
    }
}

/// Every row's fitted value from channel-major `coefficients` within
/// `tolerance` of the target, relative to `max(|fitted|, |target|, 1)`.
fn channel_major_reconstruction_passed(
    design: &crate::fe::BorrowedInterceptFeDesign<'_>,
    effects: &[CategoricalAbsorbedEffectView<'_>],
    target: &[f64],
    offsets: &[usize],
    coefficients: &[f64],
    tolerance: f64,
) -> bool {
    target.iter().enumerate().all(|(row, expected)| {
        let mut fitted = 0.0;
        for (index, (effect, entry)) in effects.iter().zip(design.effects()).enumerate() {
            let level = entry.codes()[row] as usize;
            for channel in 0..effect.channels() {
                let basis = if effect.intercept && channel == 0 {
                    1.0
                } else {
                    effect.slopes[channel - usize::from(effect.intercept)][row]
                };
                fitted += coefficients[offsets[index] + channel * entry.n_levels() + level] * basis;
            }
        }
        let scale = fitted.abs().max(expected.abs()).max(1.0);
        fitted.is_finite() && (fitted - expected).abs() / scale <= tolerance
    })
}
