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

mod integer_dual;

// Keep the certified proposal inline: boxing would add an unadmitted heap
// allocation and alter the detector proof's allocation/lifetime envelope.
#[allow(clippy::large_enum_variant)]
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
    admission: DetectorAdmission<'_>,
    iteration_limit: usize,
) -> Result<Resolution> {
    let (ppml, linear) = admission.unit_projection.unwrap_or((ppml, linear));
    let (ppml, linear) = strict_projection_policy(ppml, linear);
    let outcome = generation.outcome();
    let unit = fallible_filled(outcome.len(), 1., "unable to reserve unit dual weights")?;
    let mut response =
        fallible_filled(outcome.len(), 0.0_f64, "unable to reserve dual recurrence")?;
    // Only the categorical audit can retain the admitted linear panel. It
    // has no slope workspace competing with that storage between projections.
    let retain_within = admission.unit_projection.is_some()
        && generation.model().columns.len() > 1
        && generation
            .prepared()
            .as_ref()
            .effect_specs()
            .iter()
            .all(|spec| spec.slopes.is_empty());
    let mut design = None;
    let mut unresolved_margin = 0.;
    let limit = iteration_limit.min(config.max_ir_iterations);
    let mut progress = Progress::new();
    let exit = Exit(generation, (ppml, linear), config, admission);
    for iteration in 1..=limit {
        interrupt(PpmlInterruptPoint::SeparationIrIteration)?;
        for (value, &y) in response.iter_mut().zip(outcome) {
            if y == 0. {
                *value = (*value).max(1.);
            }
        }
        let project = |slot: Option<&mut Option<_>>| {
            prepared_project_with(
                generation, &response, &unit, ppml, config, linear, None, false, slot,
            )
        };
        let reused = design.is_some();
        let projection = match project(retain_within.then_some(&mut design)) {
            Err(error) if reused && is_numerical_refusal(&error) => {
                drop(design.take());
                project(Some(&mut design))?
            }
            result => result?,
        };
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
            admission.span_plan,
            &mut design,
        )? {
            return Ok(Resolution::Dual(dual, iteration));
        }
        let mut direction = projection.fitted;
        if let Some(proposal) = primal_proposal(generation, config, iteration, &mut direction)? {
            return Ok(Resolution::Direction(proposal));
        }
        // A nearly exact primal separator can leave a tiny, positive
        // projection residual made only of roundoff. Test the independent
        // primal certificate above before interpreting that residual as an
        // unresolved dual; its relative margin alone cannot distinguish it.
        // A small first positive margin makes response-relative projection
        // error visible. Before refusing, try an exact lattice witness, then
        // remove the measured span component with a fresh projection. Both
        // must meet the same full-model certificate used at terminal replay.
        let margin = outcome
            .iter()
            .zip(&dual)
            .filter_map(|(&y, &value)| (y == 0.).then_some(value))
            .fold(f64::INFINITY, f64::min);
        // Measured progress toward either certificate. When neither can be
        // reached within the remaining budget at the measured rates, try an
        // exact lane that has not run on this generation, then stop.
        progress.observe(margin, primal_violation(outcome, &direction));
        if let Some(stall) =
            progress.stalled(iteration, limit - iteration, config.certificate_tolerance)
        {
            drop(design.take());
            return switch_or_stop(exit, interrupt, (stall, iteration));
        }
        let relative_margin = margin / stable_norm(&dual);
        if margin.is_finite() && relative_margin > config.certificate_tolerance {
            for (integer, &value) in direction.iter_mut().zip(&dual) {
                *integer = (value / margin).round();
            }
            if integer_dual::certify(generation, &direction, config.certificate_tolerance)? {
                return Ok(Resolution::Dual(direction, iteration));
            }
            drop(direction);
            drop(design.take());
            let refined =
                prepared_project(generation, &dual, &unit, ppml, config, linear)?.residual;
            drop(dual);
            if certify_dual_with_weights(
                generation,
                &refined,
                &unit,
                ppml,
                config,
                linear,
                admission.span_plan,
                &mut design,
            )? {
                return Ok(Resolution::Dual(refined, iteration));
            }
            // The first strictly positive margin can be too small to resolve
            // scale-dependent roundoff. Continue the cone recurrence while its
            // normalized interior margin improves. Stagnation preserves the
            // numerical refusal for an unproved or truncated model span.
            if relative_margin <= unresolved_margin * (1. + config.projection_tolerance) {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "unit-metric separation dual has unresolved full-model orthogonality",
                ));
            }
            unresolved_margin = relative_margin;
            response = refined;
            continue;
        }
        response = dual;
    }
    if limit >= config.max_ir_iterations {
        // A spent full budget is final, like a stall: no solver-variant
        // restart repeats this recurrence.
        drop(design.take());
        return switch_or_stop(exit, interrupt, (progress.exhausted(limit), limit));
    }
    Err(FeregError::new(
        ErrorCode::NonConvergence,
        "unit-metric separation check exhausted its primal/dual iteration budget",
    ))
}

/// What a stalled or spent recurrence needs to switch lanes or stop.
#[derive(Clone, Copy)]
struct Exit<'g, 'a>(
    &'g PreparedPpmlModelGeneration,
    (PpmlConfig, PreparedOlsFitConfig),
    SeparationConfig,
    DetectorAdmission<'a>,
);

/// After a stall or a spent budget: one streamed exact lane not yet run on
/// this generation, then the explicit, final stall error.
fn switch_or_stop(
    exit: Exit<'_, '_>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    (stall, iteration): (Stall, usize),
) -> Result<Resolution> {
    let Exit(generation, projections, config, admission) = exit;
    match super::streamed_switch(generation, projections, config, admission, interrupt)? {
        Some(super::ExactSwitch::Direction(proposal)) => Ok(Resolution::Direction(proposal)),
        Some(super::ExactSwitch::Dual(dual)) => Ok(Resolution::Dual(dual, iteration)),
        None => Err(stall.error(generation.generation(), iteration, admission)),
    }
}

/// Prefix of the explicit, non-retryable stall error. Re-running the same
/// recurrence with another projection solver repeats the same geometry.
pub(super) const STALLED: &str = "unit-metric separation recurrence stalled";

pub(super) fn is_stall(error: &FeregError) -> bool {
    error.code == ErrorCode::NonConvergence && error.message.starts_with(STALLED)
}

/// Largest normalized violation of the primal certificate: any value on a
/// positive row and any positive value on a zero row, after the proposal's
/// own unit max-norm scaling. A zero direction has no primal progress.
fn primal_violation(outcome: &[f64], direction: &[f64]) -> f64 {
    let scale = infinity_norm(direction);
    if scale.is_nan() || scale <= f64::MIN_POSITIVE || (scale - 1.).abs() > 1e-12 {
        return f64::INFINITY;
    }
    outcome
        .iter()
        .zip(direction)
        .map(|(&y, &z)| if y > 0. { z.abs() } else { z.max(0.) })
        .fold(0., f64::max)
}

/// Best margin and best primal violation, compared at doubling checkpoints.
/// The margin must rise to the certificate's positive interior; the primal
/// violation must fall to the certificate tolerance. Each projected count
/// assumes the rate measured since the previous checkpoint continues:
/// linear for the margin and a power law for the violation.
struct Progress {
    best_margin: f64,
    best_violation: f64,
    reference: Option<(usize, f64, f64)>,
    next: usize,
}

pub(super) struct Stall {
    margin: f64,
    margin_rate: f64,
    violation: f64,
    exponent: f64,
    remaining: usize,
}

impl Progress {
    /// Recorded first after this many projections, then compared at twice it.
    const REFERENCE: usize = 16;

    fn new() -> Self {
        Self {
            best_margin: f64::NEG_INFINITY,
            best_violation: f64::INFINITY,
            reference: None,
            next: Self::REFERENCE,
        }
    }

    fn observe(&mut self, margin: f64, violation: f64) {
        if margin.is_finite() {
            self.best_margin = self.best_margin.max(margin);
        }
        if !violation.is_nan() {
            self.best_violation = self.best_violation.min(violation);
        }
    }

    /// The measured state after a spent budget, for the final error.
    fn exhausted(&self, iteration: usize) -> Stall {
        let (margin_rate, exponent) = match self.reference {
            Some((k0, m0, v0)) if iteration > k0 => (
                (self.best_margin - m0) / (iteration - k0) as f64,
                (v0 / self.best_violation).ln() / (iteration as f64 / k0 as f64).ln(),
            ),
            _ => (f64::NAN, f64::NAN),
        };
        Stall {
            margin: self.best_margin,
            margin_rate,
            violation: self.best_violation,
            exponent,
            remaining: 0,
        }
    }

    fn stalled(&mut self, iteration: usize, remaining: usize, tolerance: f64) -> Option<Stall> {
        if iteration < self.next {
            return None;
        }
        let current = (iteration, self.best_margin, self.best_violation);
        self.next = iteration.saturating_mul(2);
        let previous = self.reference.replace(current)?;
        let (k0, m0, v0) = previous;
        let (k1, m1, v1) = current;
        if m1 >= 0. || v1 <= tolerance {
            return None;
        }
        let margin_rate = (m1 - m0) / (k1 - k0) as f64;
        let margin_needs = if margin_rate > 0. {
            -m1 / margin_rate
        } else {
            f64::INFINITY
        };
        let exponent = (v0 / v1).ln() / (k1 as f64 / k0 as f64).ln();
        let primal_needs = if exponent > 0. && v1.is_finite() {
            k1 as f64 * ((v1 / tolerance).powf(1. / exponent) - 1.)
        } else {
            f64::INFINITY
        };
        let budget = remaining as f64;
        (margin_needs > budget && primal_needs > budget).then_some(Stall {
            margin: m1,
            margin_rate,
            violation: v1,
            exponent,
            remaining,
        })
    }
}

impl Stall {
    fn error(
        &self,
        generation: usize,
        iteration: usize,
        admission: DetectorAdmission<'_>,
    ) -> FeregError {
        FeregError::new(
            ErrorCode::NonConvergence,
            format!(
                "{STALLED} on generation {generation} after {iteration} projections: best zero-row margin {:.3e} (measured rate {:.3e} per projection), best primal violation {:.3e} (measured power {:.3}); neither certificate is reachable in the remaining {} projections; exact lanes tried: streamed compact simplex ({})",
                self.margin,
                self.margin_rate,
                self.violation,
                self.exponent,
                self.remaining,
                if admission.exact_tried { "in the simplex phase" } else { "after the stall" },
            ),
        )
    }
}

/// Test the fitted primal direction before interpreting its dual residual.
fn primal_proposal(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    iteration: usize,
    direction: &mut Vec<f64>,
) -> Result<Option<PreparedNumericalProposal>> {
    direction_proposal(
        generation,
        config,
        iteration,
        direction,
        "prepared iterated-rectifier candidate direction",
    )
}

/// An IR deletion from a direction that is positive on the rows it drops:
/// zero on positive rows and nonpositive on zero rows to the certificate
/// tolerance, after unit max-norm scaling.
pub(super) fn direction_proposal(
    generation: &PreparedPpmlModelGeneration,
    config: SeparationConfig,
    iteration: usize,
    direction: &mut Vec<f64>,
    note: &'static str,
) -> Result<Option<PreparedNumericalProposal>> {
    let outcome = generation.outcome();
    let scale = infinity_norm(direction);
    if scale > f64::MIN_POSITIVE {
        direction
            .iter_mut()
            .for_each(|value| *value = -*value / scale);
        let dropped = fallible_collect(
            outcome
                .iter()
                .zip(direction.iter())
                .enumerate()
                .filter_map(|(row, (&y, &z))| (y == 0. && z < -config.tolerance).then_some(row)),
            outcome.len(),
            "unable to reserve unit-metric separation rows",
        )?;
        if !dropped.is_empty()
            && outcome.iter().zip(direction.iter()).all(|(&y, &z)| {
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
                direction,
                &dropped,
                config.certificate_tolerance,
            )?;
            if certificate.passed {
                return Ok(Some(bind_numerical_proposal(
                    generation,
                    DetectionProposal {
                        method: SeparationMethod::IteratedRectifier,
                        dropped,
                        iterations: iteration,
                        status: MethodStatus::Certified,
                        certificate: Some(certificate),
                        exact_fe_certificate: None,
                        direction: Some(std::mem::take(direction)),
                        note: note.into(),
                    },
                )?));
            }
        }
    }
    Ok(None)
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
    admission: DetectorAdmission<'_>,
) -> Result<bool> {
    if integer_dual::certify(generation, dual, config.certificate_tolerance)? {
        return Ok(true);
    }
    let unit = fallible_filled(dual.len(), 1., "unable to reserve dual certificate weights")?;
    if let Some((fast_ppml, fast_linear)) = fast_unit_policy(generation, ppml, linear, admission) {
        match certify_dual_with_weights(
            generation,
            dual,
            &unit,
            fast_ppml,
            config,
            fast_linear,
            admission.span_plan,
            &mut None,
        ) {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(error) if is_numerical_refusal(&error) => {}
            Err(error) => return Err(error),
        }
    }
    let (ppml, linear) = admission.unit_projection.unwrap_or((ppml, linear));
    certify_dual_with_weights(
        generation,
        dual,
        &unit,
        ppml,
        config,
        linear,
        admission.span_plan,
        &mut None,
    )
}

#[allow(clippy::too_many_arguments)]
fn certify_dual_with_weights<'a>(
    generation: &'a PreparedPpmlModelGeneration,
    dual: &[f64],
    unit: &[f64],
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    span_plan: Option<&LiveMemoryPlan>,
    design: &mut Option<crate::ols::UnitProjectionDesign<'a>>,
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
        span_plan,
    )? {
        trace_dual_check("local", margin, 0., config.projection_tolerance);
        return Ok(false);
    }
    // A missing margin leaves the design with the recurrence. Once there is
    // a margin, consume it in the projected check and release it before any
    // individual omitted-column projection or full raw retry.
    if let Some(design) = design.take() {
        let reused = certify_projected_dual(
            generation,
            dual,
            unit,
            margin,
            ppml,
            config,
            linear,
            Some(design),
        );
        match reused {
            Ok(true) => return Ok(true),
            Err(error) if !is_numerical_refusal(&error) => return Err(error),
            _ => {}
        }
    }
    // A rejected single-response projection or span audit retries the original
    // joint projection. Terminal replay always comes directly through here.
    certify_projected_dual(generation, dual, unit, margin, ppml, config, linear, None)
}

#[allow(clippy::too_many_arguments)]
fn certify_projected_dual<'a>(
    generation: &'a PreparedPpmlModelGeneration,
    dual: &[f64],
    unit: &[f64],
    margin: f64,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    mut design: Option<crate::ols::UnitProjectionDesign<'a>>,
) -> Result<bool> {
    let mut aliases = Vec::new();
    let checked = prepared_project_with(
        generation,
        dual,
        unit,
        ppml,
        config,
        linear,
        Some(&mut |relations, norms| {
            certify_qr_aliases(generation, relations, norms, &mut aliases)
        }),
        false,
        design.is_some().then_some(&mut design),
    )?;
    drop(design);
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
    certify_omitted_contrasts(
        generation, dual, unit, &kept, &aliases, margin, ppml, config, linear,
    )
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

/// Use the QR's discarded-column loadings only as proposals. Independently
/// prove every accepted alias from original values and FE memberships. The
/// verification runs again when the separation coordinator replays the proof.
fn certify_qr_aliases(
    generation: &PreparedPpmlModelGeneration,
    relations: crate::ols::finite_operations::BasisRelations<'_>,
    norms: &[f64],
    certified: &mut Vec<usize>,
) -> Result<()> {
    certified.clear();
    let rank = relations.selected.len();
    let dropped = relations.candidates.len() - rank;
    if dropped == 0 {
        return Ok(());
    }
    let fit = generation.fit_view();
    let rows = generation.outcome().len();
    let mut response = fallible_filled(rows, 0., "unable to reserve omitted contrast")?;
    let mut coefficients = fallible_filled(rank, (0, 0.), "unable to reserve QR alias proposal")?;
    certified
        .try_reserve_exact(dropped)
        .map_err(|_| allocation("unable to reserve verified QR aliases"))?;
    for (position, &dropped_candidate) in relations.permutation[rank..].iter().enumerate() {
        crate::interrupt::checkpoint()?;
        let column = relations.candidates[dropped_candidate];
        for (pivot, entry) in coefficients.iter_mut().enumerate() {
            let kept = relations.candidates[relations.permutation[pivot]];
            let gamma = relations.dropped_projection[pivot * dropped + position];
            *entry = (kept, gamma * norms[column] / norms[kept]);
        }
        crate::ols::prepared_fit_view::visit_prepared_column_range(
            fit,
            column,
            0..rows,
            |row, value| {
                crate::interrupt::row_checkpoint(row)?;
                response[row] = value;
                Ok(())
            },
        )?;
        if exact_integer_alias(generation, &mut response, &coefficients, relations.selected)? {
            certified.push(column);
        }
    }
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=separation_qr_alias|verified={}|candidates={dropped}",
            certified.len(),
        ));
    }
    Ok(())
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
    certified_aliases: &[usize],
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
    // Ordered proof invariant: every lower index is retained or already
    // proved redundant. An unresolved omission returns before advancing;
    // only this certified prefix may supply additional exact-copy witnesses.
    for column in 0..fit.model().columns.len() {
        crate::interrupt::checkpoint()?;
        if kept.contains(&column)
            || certified_aliases.contains(&column)
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

/// A complete dual of the original model proves no separation under any
/// positive search metric. Leave positive proposals to the existing detector.
pub(super) fn probe_dual(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    admission: DetectorAdmission<'_>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<PreparedNumericalProposal>> {
    if let Some(policy) = fast_unit_policy(generation, ppml, linear, admission) {
        let trial = DetectorAdmission {
            unit_projection: Some(policy),
            ..admission
        };
        match resolve(generation, ppml, config, linear, interrupt, trial, 1) {
            Ok(Resolution::Dual(dual, iterations)) => {
                return bound_dual(generation, dual, iterations).map(Some);
            }
            Ok(Resolution::Direction(_)) => return Ok(None),
            Err(error) if is_numerical_refusal(&error) => {}
            Err(error) => return Err(error),
        }
    }
    match resolve(generation, ppml, config, linear, interrupt, admission, 1) {
        Ok(Resolution::Dual(dual, iterations)) => {
            bound_dual(generation, dual, iterations).map(Some)
        }
        Ok(Resolution::Direction(_)) => Ok(None),
        Err(error) if is_numerical_refusal(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn is_numerical_refusal(error: &FeregError) -> bool {
    matches!(
        error.code,
        ErrorCode::NonConvergence | ErrorCode::InvalidData
    )
}

/// Use exact elimination or bounded Auto only on a design it can serve.
/// This speculative cap leaves the admitted unit LSMR and weighted budgets
/// intact. The new response and terminal replay rebuild their own solves.
fn fast_unit_policy(
    generation: &PreparedPpmlModelGeneration,
    mut ppml: PpmlConfig,
    mut linear: PreparedOlsFitConfig,
    admission: DetectorAdmission<'_>,
) -> Option<(PpmlConfig, PreparedOlsFitConfig)> {
    let effects = generation.prepared().as_ref().effect_specs();
    if admission.unit_projection.is_none()
        || ppml.absorb.solver != FeSolverKind::Auto
        || !(2..=5).contains(&effects.len())
        || effects.iter().any(|spec| !spec.slopes.is_empty())
    {
        return None;
    }
    linear.absorb.map.max_iterations = linear.absorb.map.max_iterations.min(128);
    // Reserve the full LSMR budget for the established strict attempt.
    linear.absorb.lsmr.max_iterations = linear.absorb.lsmr.max_iterations.min(128);
    ppml.absorb = linear.absorb;
    Some((ppml, linear))
}

/// Certify an externally proposed dual with the complete unit-metric check.
/// A numerical refusal leaves the ordinary detector in control.
pub(super) fn certify_hint(
    generation: &PreparedPpmlModelGeneration,
    dual: &[f64],
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    admission: DetectorAdmission<'_>,
) -> Result<Option<PreparedNumericalProposal>> {
    let numerical = |result: Result<bool>| match result {
        Ok(certified) => Ok(certified),
        Err(error) if is_numerical_refusal(&error) => Ok(false),
        Err(error) => Err(error),
    };
    let mut certified = numerical(certify_dual(
        generation, dual, ppml, config, linear, admission,
    ))?;
    let mut refined = None;
    if !certified {
        // A least-squares lift balances each column to roundoff, which adds up
        // over many rows. Remove the measured span component with one fresh
        // unit projection, as the recurrence does, and certify that vector.
        let (unit_ppml, unit_linear) = admission.unit_projection.unwrap_or((ppml, linear));
        let (unit_ppml, unit_linear) = strict_projection_policy(unit_ppml, unit_linear);
        let unit = fallible_filled(dual.len(), 1., "unable to reserve hint refinement weights")?;
        let projected =
            match prepared_project(generation, dual, &unit, unit_ppml, config, unit_linear) {
                Ok(projection) => Some(projection.residual),
                Err(error) if is_numerical_refusal(&error) => None,
                Err(error) => return Err(error),
            };
        if let Some(vector) = projected {
            certified = numerical(certify_dual(
                generation, &vector, ppml, config, linear, admission,
            ))?;
            refined = Some(vector);
        }
    }
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=compact_dual_ir|generation={}|rows={}|refined={}|certified={}",
            generation.generation(),
            dual.len(),
            u8::from(refined.is_some()),
            u8::from(certified)
        ));
    }
    if !certified {
        return Ok(None);
    }
    let owned = match refined {
        Some(vector) => vector,
        None => fallible_collect(
            dual.iter().copied(),
            dual.len(),
            "unable to reserve IR dual",
        )?,
    };
    bound_dual(generation, owned, 1).map(Some)
}

fn bound_dual(
    generation: &PreparedPpmlModelGeneration,
    dual: Vec<f64>,
    iterations: usize,
) -> Result<PreparedNumericalProposal> {
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

/// A weighted numerical failure can lose a boundary-only direction before
/// returning any proposal. The same unit-metric certificate is the bounded
/// recovery; a failed recovery preserves the original numerical error.
#[allow(clippy::too_many_arguments)]
pub(super) fn recover(
    generation: &PreparedPpmlModelGeneration,
    ppml: PpmlConfig,
    config: SeparationConfig,
    linear: PreparedOlsFitConfig,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
    original: FeregError,
    admission: DetectorAdmission<'_>,
    iterations: usize,
) -> Result<PreparedNumericalProposal> {
    match resolve(
        generation, ppml, config, linear, interrupt, admission, iterations,
    ) {
        Ok(Resolution::Direction(proposal)) => Ok(proposal),
        Ok(Resolution::Dual(dual, iterations)) => bound_dual(generation, dual, iterations),
        // A full-budget recovery that stalled is final; a short probe keeps
        // the original error so its caller's ordinary fallback still runs.
        Err(error) if is_stall(&error) && iterations >= config.max_ir_iterations => Err(error),
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
/// Call only after every preceding column has an independent span proof.
pub(super) fn exact_finite_duplicate(
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
        // The caller visits columns in order and aborts on any unproved
        // omission. Every earlier column therefore already belongs to the
        // certified retained-plus-FE span, even if itself numerically omitted.
        // These exact identities form an acyclic proof: never use the current
        // column or a future omission. The ORIGINAL kept set still supplies
        // constant authority above and the projection-basis guard in the caller.
        let proved = kept
            .iter()
            .copied()
            .chain((0..column).filter(|other| !kept.contains(other)));
        for other in proved {
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

#[cfg(test)]
mod progress_tests {
    use super::Progress;

    #[test]
    fn diverging_margin_and_harmonic_primal_decay_stall_at_the_first_comparison() {
        let mut progress = Progress::new();
        let mut stalled = None;
        for k in 1..=40usize {
            // Margin drifts away from the interior; the primal violation
            // falls like 1/k, far too slowly for a 1e-8 certificate.
            progress.observe(-0.03 - 0.01 * k as f64, 0.5 / k as f64);
            if let Some(stall) = progress.stalled(k, 10_000 - k, 1e-8) {
                stalled = Some((k, stall.remaining));
                break;
            }
        }
        assert_eq!(stalled, Some((32, 10_000 - 32)));
    }

    #[test]
    fn an_improving_margin_that_reaches_the_interior_within_budget_continues() {
        let mut progress = Progress::new();
        for k in 1..=200usize {
            progress.observe(-0.8 + 0.004 * k as f64, 0.5);
            assert!(progress.stalled(k, 10_000 - k, 1e-8).is_none(), "{k}");
        }
    }

    #[test]
    fn a_missed_checkpoint_still_fires_on_a_later_projection() {
        let mut progress = Progress::new();
        for k in 1..=31usize {
            progress.observe(-0.5, 0.4);
            if k != 16 {
                continue;
            }
            assert!(progress.stalled(k, 1000, 1e-8).is_none());
        }
        // Projection 32 took the positive-margin branch and skipped the test.
        progress.observe(-0.5, 0.4);
        progress.observe(-0.5, 0.4);
        assert!(progress.stalled(33, 1000, 1e-8).is_some());
    }

    #[test]
    fn a_short_budget_declares_a_slow_but_real_improvement_unreachable() {
        let mut progress = Progress::new();
        let mut fired = false;
        for k in 1..=64usize {
            progress.observe(-0.7 + 0.0005 * k as f64, 0.3);
            fired |= progress.stalled(k, 128 - k, 1e-8).is_some();
        }
        assert!(fired);
    }
}
