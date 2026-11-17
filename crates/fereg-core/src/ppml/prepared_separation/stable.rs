//! Unit-metric primal/dual check of a weighted rectifier's no-drop claim.
//!
//! Let S be the complete finite-plus-FE span and C the cone of vectors zero
//! on positive outcomes and nonnegative on zeros. Dykstra projects the zero-
//! outcome indicator u onto S intersect C. Its dual residual p is in S^perp;
//! eliminating the other correction gives the recurrence
//!     t[positive] = p[positive], t[zero] = max(p[zero], 1),
//!     p_new = t - P_S(t).
//! A primal direction P_S(t) is accepted only by the ordinary independent
//! deletion certificate. A dual strictly positive on ALL zero outcomes rules
//! out every nonzero cone direction (Gordan's alternative). All projections
//! here have unit weights: no boundary-only contrast is attenuated by tol^2/N.

use super::*;

pub(super) enum Resolution {
    Dual(Vec<f64>, usize),
    Direction(PreparedNumericalProposal),
}

pub(super) fn resolve(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    full_span_bytes: u128,
) -> Result<Resolution> {
    let (ppml, linear) = strict_projection_policy(ppml, linear);
    let outcome = generation.outcome();
    let unit = fallible_filled(outcome.len(), 1., "unable to reserve unit dual weights")?;
    let mut response =
        fallible_filled(outcome.len(), 0.0_f64, "unable to reserve dual recurrence")?;
    for iteration in 1..=config.max_ir_iterations {
        interrupt(PpmlInterruptPoint::SeparationIrIteration)?;
        for (value, &y) in response.iter_mut().zip(outcome) {
            if y == 0. {
                *value = (*value).max(1.);
            }
        }
        let projection = prepared_project(generation, &response, &unit, ppml, config, linear)?;
        // Release the previous recurrence before the independent projection.
        // Peak detector storage is eight N-vectors: the weighted evidence's
        // two, unit weights, this projection's two, and an omitted-column
        // response plus its two projection outputs. The check reuses these
        // unit weights; the shared linear workspace is separate.
        drop(response);
        drop(projection.coefficients);
        let dual = projection.residual;
        if certify_dual_with_weights(
            generation,
            &dual,
            &unit,
            ppml,
            config,
            linear,
            full_span_bytes,
        )? {
            return Ok(Resolution::Dual(dual, iteration));
        }
        // Every exact projection residual is already in the full orthogonal
        // complement. A comfortably positive dual failing its span audit is
        // therefore numerical ambiguity, not a cone-feasibility iteration.
        // Repeating the same truncated projector cannot repair that evidence.
        let margin = outcome
            .iter()
            .zip(&dual)
            .filter_map(|(&y, &value)| (y == 0.).then_some(value))
            .fold(f64::INFINITY, f64::min);
        if margin.is_finite() && margin > config.certificate_tolerance * stable_norm(&dual) {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "unit-metric separation dual has unresolved full-model orthogonality",
            ));
        }
        let mut direction = projection.fitted;
        let scale = infinity_norm(&direction);
        if scale > f64::MIN_POSITIVE {
            direction
                .iter_mut()
                .for_each(|value| *value = -*value / scale);
            let dropped = fallible_collect(
                outcome
                    .iter()
                    .zip(&direction)
                    .enumerate()
                    .filter_map(|(row, (&y, &z))| {
                        (y == 0. && z < -config.tolerance).then_some(row)
                    }),
                outcome.len(),
                "unable to reserve unit-metric separation rows",
            )?;
            if !dropped.is_empty()
                && outcome.iter().zip(&direction).all(|(&y, &z)| {
                    if y > 0. {
                        z.abs() <= config.certificate_tolerance
                    } else {
                        z <= config.certificate_tolerance
                    }
                })
                && dropped
                    .iter()
                    .all(|&row| -direction[row] >= config.certificate_tolerance)
            {
                let certificate = detector_direction_certificate(
                    outcome,
                    &direction,
                    &dropped,
                    config.certificate_tolerance,
                )?;
                if certificate.passed {
                    return Ok(Resolution::Direction(bind_numerical_proposal(
                        generation,
                        DetectionProposal {
                            method: SeparationMethod::IteratedRectifier,
                            dropped,
                            iterations: iteration,
                            status: MethodStatus::Certified,
                            certificate: Some(certificate),
                            exact_fe_certificate: None,
                            direction: Some(direction),
                            note: "prepared iterated-rectifier candidate direction".into(),
                        },
                    )?));
                }
            }
        }
        response = dual;
    }
    Err(FeregError::new(
        ErrorCode::NonConvergence,
        "unit-metric separation check exhausted its primal/dual iteration budget",
    ))
}

/// Check the full model, including finite columns omitted by the weighted
/// detector. Normalize errors by the smallest boundary margin, never by the
/// potentially much larger positive-row multipliers. A second unit projection
/// checks the actual span component as well as the raw channel moments.
pub(super) fn certify_dual(
    generation: &PreparedPpmlModelGeneration,
    dual: &[f64],
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    full_span_bytes: u128,
) -> Result<bool> {
    let unit = fallible_filled(dual.len(), 1., "unable to reserve dual certificate weights")?;
    certify_dual_with_weights(
        generation,
        dual,
        &unit,
        ppml,
        config,
        linear,
        full_span_bytes,
    )
}

fn certify_dual_with_weights(
    generation: &PreparedPpmlModelGeneration,
    dual: &[f64],
    unit: &[f64],
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    full_span_bytes: u128,
) -> Result<bool> {
    let (ppml, linear) = strict_projection_policy(ppml, linear);
    if dual.len() != generation.outcome().len() || dual.iter().any(|v| !v.is_finite()) {
        return Ok(false);
    }
    let margin = generation
        .outcome()
        .iter()
        .zip(dual)
        .filter_map(|(&y, &v)| (y == 0.).then_some(v))
        .fold(f64::INFINITY, f64::min);
    if !margin.is_finite() || margin <= config.certificate_tolerance * stable_norm(dual) {
        trace_dual_check(
            "margin",
            margin,
            stable_norm(dual),
            config.certificate_tolerance,
        );
        return Ok(false);
    }
    let columns = fallible_collect(
        0..generation.fit_view().model().columns.len(),
        generation.fit_view().model().columns.len(),
        "unable to reserve complete dual column identities",
    )?;
    let moment = ir_regressor_moment_ratio(generation, &|_| 1., dual, &columns)?
        .max(ir_fe_moment_ratio(generation, &|_| 1., dual)?)
        .max(centered_moment_ratio(generation, dual)?);
    if moment > config.projection_tolerance * margin {
        trace_dual_check("moment", margin, moment, config.projection_tolerance);
        return Ok(false);
    }
    if !stable_local::certify(
        generation,
        dual,
        margin,
        config.projection_tolerance,
        config.max_ir_iterations.saturating_mul(
            ppml.absorb
                .map
                .max_iterations
                .max(ppml.absorb.lsmr.max_iterations),
        ),
        full_span_bytes,
    )? {
        trace_dual_check("local", margin, 0., config.projection_tolerance);
        return Ok(false);
    }
    let checked = prepared_project(generation, dual, unit, ppml, config, linear)?;
    if stable_norm(&checked.fitted) > config.projection_tolerance * margin {
        trace_dual_check(
            "span",
            margin,
            stable_norm(&checked.fitted),
            config.projection_tolerance,
        );
        return Ok(false);
    }
    let kept = checked.kept_columns;
    drop(checked.fitted);
    drop(checked.residual);
    drop(checked.coefficients);
    certify_omitted_contrasts(generation, dual, unit, &kept, margin, ppml, config, linear)
}

/// Intercept-preserving translations must not hide an omitted model direction
/// behind a huge raw column norm. These centered channels belong to exactly
/// the same full span, and are checked even if the projection omitted them.
fn centered_moment_ratio(generation: &PreparedPpmlModelGeneration, dual: &[f64]) -> Result<f64> {
    let fit = generation.fit_view();
    let mut largest = 0.0_f64;
    if fit.fit_structural_intercept() || fit.fit_has_absorbed_intercept() {
        for column in 0..fit.model().columns.len() {
            crate::interrupt::checkpoint()?;
            let anchor = fit.column_value(column, 0)?;
            let (mut sum, mut correction) = (0., 0.);
            let mut norm = ScaledSquares::ZERO;
            crate::ols::prepared_fit_view::visit_prepared_column_range(
                fit,
                column,
                0..dual.len(),
                |row, value| {
                    crate::interrupt::row_checkpoint(row)?;
                    let centered = value - anchor;
                    neumaier_add(centered * dual[row], &mut sum, &mut correction);
                    norm.add(centered.abs());
                    Ok(())
                },
            )?;
            largest = ir_moment_ratio(largest, sum + correction, norm.norm());
        }
    }
    let categorical = generation.prepared().as_ref();
    let topology = DetectorTopology::new(generation.prepared())?;
    for (effect, spec) in categorical.effect_specs().iter().enumerate() {
        if !spec.intercept {
            continue;
        }
        let source = topology.sources[topology.effect_to_source[effect]];
        for slope in &spec.slopes {
            let values = categorical.numeric_source_values(slope.source)?;
            for level in 0..source.n_levels() {
                crate::interrupt::checkpoint()?;
                let members = source.checked_members(level)?;
                let Some(&first) = members.first() else {
                    continue;
                };
                let (mut sum, mut correction) = (0., 0.);
                let mut norm = ScaledSquares::ZERO;
                for (position, &row) in members.iter().enumerate() {
                    crate::interrupt::row_checkpoint(position)?;
                    let centered = values[row] - values[first];
                    neumaier_add(centered * dual[row], &mut sum, &mut correction);
                    norm.add(centered.abs());
                }
                largest = ir_moment_ratio(largest, sum + correction, norm.norm());
            }
        }
    }
    Ok(largest)
}

/// Numerical omission is not authority to erase a direction from the cone.
/// A projection only proposes alias coefficients. Every omitted column needs
/// an independent exact source identity: individually small residual moments
/// cannot certify the joint span of nearly dependent omitted contrasts.
#[allow(clippy::too_many_arguments)]
fn certify_omitted_contrasts(
    generation: &PreparedPpmlModelGeneration,
    dual: &[f64],
    unit: &[f64],
    kept: &[usize],
    margin: f64,
    mut ppml: PpmlConfig,
    config: SeparationConfig,
    mut linear: PreparedOlsFitConfig,
) -> Result<bool> {
    let fit = generation.fit_view();
    ppml.absorb.map.tolerance = ppml.absorb.map.tolerance.min(1e-14);
    ppml.absorb.lsmr.tolerance = ppml.absorb.lsmr.tolerance.min(1e-14);
    linear.absorb.map.tolerance = ppml.absorb.map.tolerance;
    linear.absorb.lsmr.tolerance = ppml.absorb.lsmr.tolerance;
    for column in 0..fit.model().columns.len() {
        crate::interrupt::checkpoint()?;
        if kept.contains(&column)
            || exact_absorbed_column(generation, column)?
            || exact_finite_duplicate(generation, column, kept)?
        {
            continue;
        }
        let anchor = if constant_in_certified_span(generation, kept) {
            fit.column_value(column, 0)?
        } else {
            0.
        };
        let mut response = fallible_filled(dual.len(), 0., "unable to reserve omitted contrast")?;
        let mut lost_information = false;
        crate::ols::prepared_fit_view::visit_prepared_column_range(
            fit,
            column,
            0..dual.len(),
            |row, value| {
                crate::interrupt::row_checkpoint(row)?;
                response[row] = value - anchor;
                let virtual_anchor = value - response[row];
                let error = (value - (response[row] + virtual_anchor)) + (virtual_anchor - anchor);
                lost_information |= error != 0.;
                Ok(())
            },
        )?;
        let scale = infinity_norm(&response);
        if scale == 0. {
            continue;
        }
        if !scale.is_finite() {
            return Ok(false);
        }
        // A power-of-two rescaling is exact except for underflow, detected
        // by the round trip. Arbitrary division could erase a weak contrast.
        let normalization = if scale < 1. || scale > (1_u64 << 53) as f64 {
            f64::from_bits((scale.to_bits() & 0x7ff0_0000_0000_0000).max(1_u64 << 52))
        } else {
            1.
        };
        for (row, value) in response.iter_mut().enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            let normalized = *value / normalization;
            lost_information |= normalized * normalization != *value;
            *value = normalized;
        }
        let mut projected = prepared_project(generation, &response, unit, ppml, config, linear)?;
        // An omitted column cannot certify itself through a changed basis.
        // Alias coefficients and residual contrasts must refer to the same
        // retained span whose orthogonality the dual check already proved.
        if projected.kept_columns != kept {
            trace_dual_check("basis", margin, 0., config.projection_tolerance);
            return Ok(false);
        }
        // Certify a proposed identity against the ORIGINAL stored column,
        // never its possibly rounded centered/scaled projection response.
        for (_, coefficient) in &mut projected.coefficients {
            *coefficient *= normalization;
        }
        crate::ols::prepared_fit_view::visit_prepared_column_range(
            fit,
            column,
            0..dual.len(),
            |row, value| {
                crate::interrupt::row_checkpoint(row)?;
                response[row] = value;
                Ok(())
            },
        )?;
        if exact_integer_alias(generation, &mut response, &projected.coefficients, kept)? {
            continue;
        }
        trace_dual_check(
            if lost_information {
                "contrast_rounding"
            } else {
                "unproved_omitted_alias"
            },
            margin,
            stable_norm(&projected.residual),
            config.projection_tolerance,
        );
        return Ok(false);
    }
    Ok(true)
}

/// Exact source/membership identities can prove an omitted column redundant
/// without interpreting projection roundoff as a new model direction.
fn exact_absorbed_column(generation: &PreparedPpmlModelGeneration, column: usize) -> Result<bool> {
    let fit = generation.fit_view();
    let categorical = generation.prepared().as_ref();
    let topology = DetectorTopology::new(generation.prepared())?;
    for (effect, spec) in categorical.effect_specs().iter().enumerate() {
        crate::interrupt::checkpoint()?;
        let source = topology.sources[topology.effect_to_source[effect]];
        if spec.intercept {
            let mut constant = true;
            for level in 0..source.n_levels() {
                crate::interrupt::checkpoint()?;
                let members = source.checked_members(level)?;
                let Some(&first) = members.first() else {
                    continue;
                };
                let value = fit.column_value(column, first)?;
                for (position, &row) in members.iter().enumerate() {
                    crate::interrupt::row_checkpoint(position)?;
                    if fit.column_value(column, row)? != value {
                        constant = false;
                        break;
                    }
                }
                if !constant {
                    break;
                }
            }
            if constant {
                return Ok(true);
            }
        }
        for slope in &spec.slopes {
            let values = categorical.numeric_source_values(slope.source)?;
            let mut identical = true;
            for (row, &value) in values.iter().enumerate() {
                crate::interrupt::row_checkpoint(row)?;
                if fit.column_value(column, row)? != value {
                    identical = false;
                    break;
                }
            }
            if identical {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn stable_norm(values: &[f64]) -> f64 {
    let mut norm = ScaledSquares::ZERO;
    for value in values {
        norm.add(value.abs());
    }
    norm.norm()
}

/// A weighted numerical failure can lose a boundary-only direction before
/// returning any proposal. The same unit-metric certificate is the bounded
/// recovery; a failed recovery preserves the original numerical error.
pub(super) fn recover(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    original: FeregError,
    full_span_bytes: u128,
) -> Result<PreparedNumericalProposal> {
    match resolve(generation, ppml, config, linear, interrupt, full_span_bytes) {
        Ok(Resolution::Direction(proposal)) => Ok(proposal),
        Ok(Resolution::Dual(dual, iterations)) => {
            let certificate = dual_certificate(generation, &dual);
            let mut proposal = bind_numerical_proposal(
                generation,
                DetectionProposal {
                    method: SeparationMethod::IteratedRectifier,
                    dropped: Vec::new(),
                    iterations,
                    status: MethodStatus::NoSeparation,
                    certificate: Some(certificate),
                    exact_fe_certificate: None,
                    direction: None,
                    note: "prepared iterated rectifier certified no separation".into(),
                },
            )?;
            proposal.stable_dual = Some(dual);
            Ok(proposal)
        }
        Err(error)
            if matches!(
                error.code,
                ErrorCode::NonConvergence | ErrorCode::InvalidData
            ) =>
        {
            Err(original)
        }
        Err(error) => Err(error),
    }
}

/// Scale a positive dual to boundary minimum one: its cone projection is
/// exactly zero and its full-span projection is certified zero. This is the
/// Dykstra zero fixed point, independent of the failed weighted recurrence.
pub(super) fn dual_certificate(
    generation: &PreparedPpmlModelGeneration,
    dual: &[f64],
) -> SeparationCertificate {
    let scale = infinity_norm(dual);
    let margin = generation
        .outcome()
        .iter()
        .zip(dual)
        .filter_map(|(&y, &v)| (y == 0.).then_some(v))
        .fold(f64::INFINITY, f64::min);
    SeparationCertificate {
        proof: crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint,
        interior_zero_relative: 0.,
        boundary_violation_relative: 0.,
        span_residual_relative: 0.,
        negative_margin_relative: margin / scale,
        direction_scale: scale,
        positive_maximum: 0.,
        passed: scale.is_finite() && margin > 0. && margin.is_finite(),
    }
}

/// Unit-metric dual moments use the smallest boundary margin as their scale.
/// Ordinary response-relative FE accuracy cannot support that certificate on
/// large models; all unit solves therefore use the existing strict floor.
fn strict_projection_policy(
    mut ppml: PpmlConfig,
    mut linear: PreparedOlsFitConfig,
) -> (PpmlConfig, PreparedOlsFitConfig) {
    ppml.absorb.map.tolerance = ppml.absorb.map.tolerance.min(1e-14);
    ppml.absorb.lsmr.tolerance = ppml.absorb.lsmr.tolerance.min(1e-14);
    linear.absorb.map.tolerance = ppml.absorb.map.tolerance;
    linear.absorb.lsmr.tolerance = ppml.absorb.lsmr.tolerance;
    (ppml, linear)
}

fn trace_dual_check(stage: &str, margin: f64, metric: f64, tolerance: f64) {
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=separation_dual_check|stage={stage}|margin={margin:.17e}|metric={metric:.17e}|tolerance={tolerance:.17e}|passed=0"
        ));
    }
}

/// Exact power-of-two affine copies are redundant even for fractional data.
/// Both subtraction and scaling are verified lossless on the original inputs.
fn exact_finite_duplicate(
    generation: &PreparedPpmlModelGeneration,
    column: usize,
    kept: &[usize],
) -> Result<bool> {
    let fit = generation.fit_view();
    let constant = constant_in_certified_span(generation, kept);
    for centered in [false, true] {
        if centered && !constant {
            continue;
        }
        for &other in kept {
            crate::interrupt::checkpoint()?;
            let left_anchor = if centered {
                fit.column_value(other, 0)?
            } else {
                0.
            };
            let right_anchor = if centered {
                fit.column_value(column, 0)?
            } else {
                0.
            };
            let mut multiple = None;
            let mut exact = true;
            for row in 0..generation.outcome().len() {
                crate::interrupt::row_checkpoint(row)?;
                let Some(left) = exact_difference(fit.column_value(other, row)?, left_anchor)
                else {
                    exact = false;
                    break;
                };
                let Some(right) = exact_difference(fit.column_value(column, row)?, right_anchor)
                else {
                    exact = false;
                    break;
                };
                if left == 0. {
                    if right != 0. {
                        exact = false;
                        break;
                    }
                    continue;
                }
                let factor = *multiple.get_or_insert(right / left);
                let bits = factor.abs().to_bits();
                let power_two = if bits >> 52 == 0 {
                    bits != 0 && bits.is_power_of_two()
                } else {
                    bits & ((1_u64 << 52) - 1) == 0
                };
                if !factor.is_finite()
                    || !power_two
                    || left * factor != right
                    || right / factor != left
                {
                    exact = false;
                    break;
                }
            }
            if exact {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn exact_difference(left: f64, right: f64) -> Option<f64> {
    let difference = left - right;
    let virtual_right = left - difference;
    let error = (left - (difference + virtual_right)) + (virtual_right - right);
    (difference.is_finite() && error == 0.).then_some(difference)
}

/// Rounded coefficients propose an integer identity; exact arithmetic and FE
/// memberships prove it independently. No near-zero residual is accepted.
pub(super) fn exact_integer_alias(
    generation: &PreparedPpmlModelGeneration,
    response: &mut [f64],
    coefficients: &[(usize, f64)],
    allowed_columns: &[usize],
) -> Result<bool> {
    let limit = (1_u64 << 53) as f64;
    if response
        .iter()
        .any(|v| !v.is_finite() || v.fract() != 0. || v.abs() > limit)
    {
        return Ok(false);
    }
    let fit = generation.fit_view();
    for &(column, coefficient) in coefficients {
        crate::interrupt::checkpoint()?;
        let integer = coefficient.round();
        if !integer.is_finite() || integer.abs() > limit {
            return Ok(false);
        }
        if integer == 0. {
            continue;
        }
        if !allowed_columns.contains(&column) {
            return Ok(false);
        }
        for (row, value) in response.iter_mut().enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            let source = fit.column_value(column, row)?;
            if source.fract() != 0. || !source.is_finite() || source.abs() > limit {
                return Ok(false);
            }
            let product = (source as i128)
                .checked_mul(integer as i128)
                .ok_or_else(|| invalid_argument("exact alias product overflow"))?;
            let difference = (*value as i128)
                .checked_sub(product)
                .ok_or_else(|| invalid_argument("exact alias sum overflow"))?;
            let integer_limit = 1_i128 << 53;
            if product.abs() > integer_limit || difference.abs() > integer_limit {
                return Ok(false);
            }
            *value = difference as f64;
        }
    }
    if response.iter().all(|value| *value == 0.) {
        return Ok(true);
    }
    if constant_in_certified_span(generation, allowed_columns)
        && response.iter().all(|value| *value == response[0])
    {
        return Ok(true);
    }
    let categorical = generation.prepared().as_ref();
    let topology = DetectorTopology::new(generation.prepared())?;
    for (effect, spec) in categorical.effect_specs().iter().enumerate() {
        if !spec.intercept {
            continue;
        }
        let source = topology.sources[topology.effect_to_source[effect]];
        let mut constant = true;
        for level in 0..source.n_levels() {
            crate::interrupt::checkpoint()?;
            let members = source.checked_members(level)?;
            let Some(&first) = members.first() else {
                continue;
            };
            for (position, &row) in members.iter().enumerate() {
                crate::interrupt::row_checkpoint(position)?;
                if response[row] != response[first] {
                    constant = false;
                    break;
                }
            }
            if !constant {
                break;
            }
        }
        if constant {
            return Ok(true);
        }
    }
    if stable_alias::exact_two_intercepts(generation, response)? {
        return Ok(true);
    }
    // Two bounded coordinate sweeps only propose integer FE coefficients.
    // Every subtraction is exact; success proves an identity in the original
    // full categorical span. Unbalanced or noninteger aliases may remain
    // unresolved, but a small residual is never interpreted as redundancy.
    for _ in 0..2 {
        for (effect, spec) in categorical.effect_specs().iter().enumerate() {
            if !spec.intercept {
                continue;
            }
            let source = topology.sources[topology.effect_to_source[effect]];
            for level in 0..source.n_levels() {
                crate::interrupt::checkpoint()?;
                let members = source.checked_members(level)?;
                if members.is_empty() {
                    continue;
                }
                let mut sum = 0_i128;
                for (position, &row) in members.iter().enumerate() {
                    crate::interrupt::row_checkpoint(position)?;
                    let Some(next) = sum.checked_add(response[row] as i128) else {
                        return Ok(false);
                    };
                    sum = next;
                }
                let count = members.len() as i128;
                let mut proposal = sum / count;
                if (sum % count).abs() >= (count + 1) / 2 {
                    proposal += sum.signum();
                }
                for (position, &row) in members.iter().enumerate() {
                    crate::interrupt::row_checkpoint(position)?;
                    let Some(value) = (response[row] as i128).checked_sub(proposal) else {
                        return Ok(false);
                    };
                    if value.abs() > (1_i128 << 53) {
                        return Ok(false);
                    }
                    response[row] = value as f64;
                }
            }
        }
        if response.iter().all(|value| *value == 0.) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn constant_in_certified_span(generation: &PreparedPpmlModelGeneration, kept: &[usize]) -> bool {
    let fit = generation.fit_view();
    fit.fit_has_absorbed_intercept()
        || kept.iter().any(|&column| {
            matches!(
                fit.model().columns[column].kind,
                crate::ols::PreparedOlsColumnKind::Intercept
            )
        })
}
