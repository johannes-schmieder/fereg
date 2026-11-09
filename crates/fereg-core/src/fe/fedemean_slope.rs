//! fedemean's full formulation for fereg's general owned designs
//! ([`FeDesign`]: categorical effects with or without varying slopes).
//!
//! The view borrows every effect's codes and slope columns (no copy).
//! Coefficients use fereg's channel-major layout (`FeCoefficientBatch`:
//! `effect_offset + channel * levels + level`), which is fedemean's public
//! layout, so they pass through unchanged. fereg keeps its side: the
//! independent certificate (SPEC §16), budgets, error mapping and reports.

use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, Result};
use crate::fe::block_schur::{fedemean_error, route};
use crate::fe::{
    certify_residuals_with_reference_norms, FeCertificate, FeCoefficientBatch, FeDesign,
    FeSolverKind, LsmrPreconditioner, SolveReport, SolveStopReason,
};

/// fedemean's view of `design`: every effect's codes, level count, intercept
/// and slope columns, borrowed.
pub(crate) fn design_view(design: &FeDesign) -> Result<fedemean::Design<'_>> {
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(design.effects().len())
        .map_err(|_| allocation("unable to reserve fedemean effect views"))?;
    for effect in design.effects() {
        effects.push(
            fedemean::Effect::new(effect.codes(), effect.n_levels())
                .with_slopes(
                    effect.has_intercept(),
                    effect.slopes().iter().map(Vec::as_slice),
                )
                .map_err(fedemean_error)?,
        );
    }
    fedemean::Design::new(design.nobs(), effects).map_err(fedemean_error)
}

/// Whether any effect of `design` has slopes (or lacks an intercept).
pub(crate) fn sloped(design: &FeDesign) -> bool {
    design
        .effects()
        .iter()
        .any(|effect| !effect.has_intercept() || !effect.slopes().is_empty())
}

/// The channel-major coefficient layout of a design, per right-hand side.
pub(crate) struct Layout {
    offsets: Vec<usize>,
    levels: Vec<usize>,
    channels: Vec<usize>,
    per_rhs: usize,
}

impl Layout {
    pub(crate) fn new(design: &FeDesign) -> Result<Self> {
        let count = design.effects().len();
        let reserve = |label| {
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(|_| allocation(label))?;
            Ok::<Vec<usize>, crate::error::FeregError>(values)
        };
        let (mut offsets, mut levels, mut channels) = (
            reserve("unable to reserve fedemean coefficient offsets")?,
            reserve("unable to reserve fedemean coefficient levels")?,
            reserve("unable to reserve fedemean coefficient channels")?,
        );
        let mut per_rhs = 0_usize;
        for effect in design.effects() {
            offsets.push(per_rhs);
            levels.push(effect.n_levels());
            channels.push(effect.channels());
            per_rhs = effect
                .n_levels()
                .checked_mul(effect.channels())
                .and_then(|width| per_rhs.checked_add(width))
                .ok_or_else(|| allocation("fedemean coefficient layout overflows"))?;
        }
        Ok(Self {
            offsets,
            levels,
            channels,
            per_rhs,
        })
    }

    /// Zero coefficients for `rhs` columns.
    pub(crate) fn zeros(&self, rhs: usize) -> Result<Vec<f64>> {
        let elements = self
            .per_rhs
            .checked_mul(rhs)
            .ok_or_else(|| allocation("fedemean coefficient batch overflows"))?;
        crate::memory::fallible_filled(elements, 0.0, "unable to reserve fedemean coefficients")
    }

    /// The batch of `values` (`rhs` columns in this layout).
    pub(crate) fn batch(&self, values: Vec<f64>, rhs: usize) -> Result<FeCoefficientBatch> {
        let copy = |values: &[usize], label| {
            let mut copy = Vec::new();
            copy.try_reserve_exact(values.len())
                .map_err(|_| allocation(label))?;
            copy.extend_from_slice(values);
            Ok::<Vec<usize>, crate::error::FeregError>(copy)
        };
        FeCoefficientBatch::from_values(
            copy(&self.channels, "unable to reserve FE coefficient channels")?,
            copy(&self.levels, "unable to reserve FE coefficient levels")?,
            rhs,
            values,
        )
    }

    fn view(&self) -> fedemean::CoefficientLayout<'_> {
        fedemean::CoefficientLayout::new(self.per_rhs, &self.offsets, &self.levels)
            .with_channels(&self.channels)
    }
}

/// One full-system projection request.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Attempt {
    pub(crate) choice: fedemean::Choice,
    /// The request the report names (Auto, or the explicit method).
    pub(crate) requested: FeSolverKind,
    /// The requested LSMR preconditioner (modified LSMR only).
    pub(crate) requested_preconditioner: Option<LsmrPreconditioner>,
    pub(crate) tolerance: f64,
    pub(crate) max_iterations: usize,
    /// fedemean declines a larger solver workspace.
    pub(crate) memory_limit: Option<u128>,
}

impl Attempt {
    /// Auto: PCG with fedemean's automatic preconditioner (with slopes, the
    /// levels' channel blocks escalating to the hybrid Schwarz when slow or
    /// stalled and two effects have intercepts).
    pub(crate) fn auto(tolerance: f64, max_iterations: usize, memory_limit: Option<u128>) -> Self {
        Self {
            choice: fedemean::Choice {
                reduction: fedemean::Reduction::Full,
                method: fedemean::Method::Pcg,
                preconditioner: fedemean::Preconditioner::Auto,
                fallback: fedemean::Fallback::Escalate,
            },
            requested: FeSolverKind::Auto,
            requested_preconditioner: None,
            tolerance,
            max_iterations,
            memory_limit,
        }
    }

    /// Modified LSMR with `preconditioner` (its automatic choice escalates
    /// as Auto's does; explicit ones never escalate).
    pub(crate) fn lsmr(
        preconditioner: LsmrPreconditioner,
        requested: FeSolverKind,
        tolerance: f64,
        max_iterations: usize,
        memory_limit: Option<u128>,
    ) -> Self {
        Self {
            choice: crate::fe::block_schur::lsmr_choice(preconditioner),
            requested,
            requested_preconditioner: Some(preconditioner),
            tolerance,
            max_iterations,
            memory_limit,
        }
    }
}

/// The result of one projection.
pub(crate) enum Outcome {
    /// The values hold the residuals; the report carries fedemean's route,
    /// work, workspace and its own certificate (callers replace it with the
    /// independent one).
    Projected(SolveReport),
    /// fedemean declined after `iterations`; the values are untouched or
    /// partially residualized with the same projection.
    Declined {
        reason: fedemean::Decline,
        iterations: usize,
    },
}

/// Project `values` in place with fedemean's full system of `design` under
/// positive `weights` (`unit`: all one). With `coefficients` (this design's
/// [`Layout`], `values.ncols()` columns), add the fitted coefficients
/// (fedemean's add semantics; after a decline with `touched`, the
/// increments of the residualized columns).
pub(crate) fn project(
    design: &FeDesign,
    (weights, unit): (&[f64], bool),
    values: &mut DenseMatrix,
    attempt: &Attempt,
    coefficients: Option<(&Layout, &mut [f64])>,
) -> Result<Outcome> {
    if weights.len() != design.nobs() || values.nrows() != design.nobs() {
        return Err(invalid_argument(
            "fedemean full-system inputs have inconsistent observation counts",
        ));
    }
    let mut projector = fedemean::Projector::new(
        design_view(design)?,
        fedemean::Settings {
            choice: attempt.choice,
            ..fedemean::Settings::default()
        },
    );
    // A developer LSMR Break witness observes the solve's polls only: the
    // structure is built first under the ordinary poll.
    let armed = attempt.choice.method == fedemean::Method::Lsmr
        && crate::fe::lsmr_break::armed(crate::fe::DeveloperLsmrBackend::Fedemean);
    let polls = std::sync::atomic::AtomicUsize::new(0);
    let witness = || crate::fe::lsmr_break::fedemean_poll(&polls);
    let poll: &(dyn Fn() -> bool + Sync) = if armed {
        projector
            .prepare(&fedemean::Control::with_cancel(
                &crate::interrupt::cancellation_requested,
            ))
            .map_err(fedemean_error)?;
        &witness
    } else {
        &crate::interrupt::cancellation_requested
    };
    let control = fedemean::Control::with_cancel(poll);
    let mut request = fedemean::Request::new(attempt.tolerance, attempt.max_iterations);
    request.memory_limit = attempt.memory_limit;
    let fe_weights = if unit {
        fedemean::Weights::Unit
    } else {
        fedemean::Weights::Values(weights)
    };
    let (nrows, ncols) = (values.nrows(), values.ncols());
    let columns =
        fedemean::ColumnsMut::new(values.as_mut_slice(), nrows, ncols).map_err(fedemean_error)?;
    let fe_coefficients =
        coefficients.map(|(layout, values)| fedemean::CoefficientsMut::new(values, layout.view()));
    let outcome = projector
        .project(fe_weights, columns, fe_coefficients, &request, &control)
        .map_err(|error| {
            if armed && error == fedemean::Error::Cancelled {
                crate::fe::lsmr_break::record_cancelled(crate::fe::DeveloperLsmrBackend::Fedemean);
            }
            fedemean_error(error)
        })?;
    match outcome {
        fedemean::Outcome::Projected(report) => {
            trace(attempt, "certified", &report.facts, Some(&report.route));
            Ok(Outcome::Projected(solve_report(attempt, &report)?))
        }
        fedemean::Outcome::Declined { reason, facts, .. } => {
            trace(attempt, "declined", &facts, None);
            Ok(Outcome::Declined {
                reason,
                iterations: facts.iterations,
            })
        }
        _ => Ok(Outcome::Declined {
            reason: fedemean::Decline::Numerical("unknown outcome"),
            iterations: 0,
        }),
    }
}

/// The result of [`absorb_certified`].
pub(crate) struct Certified {
    /// The independent certificate decides the stop reason.
    pub(crate) report: SolveReport,
    /// fedemean's decline, when it declined (the values are then the
    /// partial or untouched input, and the report uncertified).
    pub(crate) declined: Option<(fedemean::Decline, usize)>,
}

/// Project with fedemean's full system and certify the returned residuals
/// independently (SPEC §16: fereg's local-span certificate against the
/// input's reference norms) at the attempt's tolerance. A miss earns one
/// correction of the residual at a tenth of that tolerance within the
/// remaining budget; the independent certificate then decides. A memory
/// decline is an allocation error; other declines return the uncertified
/// state with the reason.
pub(crate) fn absorb_certified(
    design: &FeDesign,
    weights: (&[f64], bool),
    values: &mut DenseMatrix,
    attempt: &Attempt,
    mut coefficients: Option<(&Layout, &mut [f64])>,
) -> Result<Certified> {
    let reference = crate::fe::weighted_column_norms(values, weights.0)?;
    let first = project(
        design,
        weights,
        values,
        attempt,
        coefficients.as_mut().map(|(l, c)| (*l, &mut **c)),
    )?;
    let mut report = match first {
        Outcome::Projected(report) => report,
        Outcome::Declined {
            reason, iterations, ..
        } => {
            return declined(
                design, weights.0, values, &reference, attempt, reason, iterations,
            )
        }
    };
    let mut certification_seconds = 0.0;
    let mut independent = |values: &DenseMatrix| {
        let started = std::time::Instant::now();
        let certificate = certify_residuals_with_reference_norms(
            design,
            weights.0,
            values,
            &reference,
            attempt.tolerance,
        );
        certification_seconds += started.elapsed().as_secs_f64();
        certificate
    };
    let mut certificate = independent(values)?;
    let remaining = attempt.max_iterations.saturating_sub(report.iterations);
    if !certificate.passed && remaining > 0 {
        let correction = Attempt {
            tolerance: attempt.tolerance / 10.0,
            max_iterations: remaining,
            ..*attempt
        };
        match project(design, weights, values, &correction, coefficients)? {
            Outcome::Projected(second) => {
                report.iterations += second.iterations;
                report.setup_seconds += second.setup_seconds;
                report.solve_seconds += second.solve_seconds;
                report.solver_workspace_peak = report
                    .solver_workspace_peak
                    .max(second.solver_workspace_peak);
                report.merge_route(&second, false);
            }
            Outcome::Declined {
                reason, iterations, ..
            } => {
                return declined(
                    design,
                    weights.0,
                    values,
                    &reference,
                    attempt,
                    reason,
                    report.iterations + iterations,
                )
            }
        }
        report.warnings.push(
            "the full-system certificate missed fereg's independent certificate; corrected at a tenth of the tolerance".to_owned(),
        );
        certificate = independent(values)?;
    }
    report.stop_reason = if certificate.passed {
        SolveStopReason::Certified
    } else {
        SolveStopReason::Uncertified
    };
    report.certificate = certificate;
    report.certification_seconds = certification_seconds;
    Ok(Certified {
        report,
        declined: None,
    })
}

fn declined(
    design: &FeDesign,
    weights: &[f64],
    values: &DenseMatrix,
    reference: &[f64],
    attempt: &Attempt,
    reason: fedemean::Decline,
    iterations: usize,
) -> Result<Certified> {
    if reason == fedemean::Decline::MemoryLimit {
        return Err(allocation(
            "the fedemean full-system workspace exceeds the memory limit",
        ));
    }
    let certificate = certify_residuals_with_reference_norms(
        design,
        weights,
        values,
        reference,
        attempt.tolerance,
    )?;
    let used = if attempt.choice.method == fedemean::Method::Lsmr {
        FeSolverKind::Lsmr
    } else {
        FeSolverKind::Cg
    };
    let mut report = SolveReport {
        requested_solver: attempt.requested,
        iterations,
        requested_preconditioner: attempt.requested_preconditioner,
        used_preconditioner: attempt.requested_preconditioner.map(|p| match p {
            LsmrPreconditioner::Auto => LsmrPreconditioner::Diagonal,
            other => other,
        }),
        ..SolveReport::single_route(
            used,
            if certificate.passed {
                SolveStopReason::Certified
            } else {
                SolveStopReason::MaximumIterations
            },
            certificate,
        )
    };
    report.warnings.push(format!(
        "the fedemean full system declined after {iterations} iterations ({})",
        describe(reason)
    ));
    Ok(Certified {
        report,
        declined: Some((reason, iterations)),
    })
}

/// A decline's reason in words.
pub(crate) fn describe(reason: fedemean::Decline) -> &'static str {
    match reason {
        fedemean::Decline::OutsideRoute => "the design is outside the route",
        fedemean::Decline::Shape => "the columns do not match the design",
        fedemean::Decline::MemoryLimit => "the solver workspace exceeds the memory limit",
        fedemean::Decline::Weights => "a fixed-effect level has no positive weight",
        fedemean::Decline::Numerical(reason) => reason,
        _ => "the route declined",
    }
}

/// fereg's report of a certified fedemean projection: the request's labels,
/// the route actually used, work, workspace and fedemean's certificate.
fn solve_report(attempt: &Attempt, report: &fedemean::Report) -> Result<SolveReport> {
    let facts = report.facts;
    let lsmr = report.route.method == fedemean::MethodUsed::Lsmr;
    let used_preconditioner = lsmr.then_some(match report.route.preconditioner {
        fedemean::PreconditionerUsed::Identity => LsmrPreconditioner::Off,
        fedemean::PreconditionerUsed::Schwarz { .. } => LsmrPreconditioner::Schwarz,
        // Jacobi, and with slopes the levels' exact channel blocks.
        _ => LsmrPreconditioner::Diagonal,
    });
    let certificate = report.certificate;
    let mut solve_report = SolveReport {
        requested_solver: attempt.requested,
        iterations: facts.iterations,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::from_bytes(
            report.workspace_bytes,
        )?,
        setup_seconds: facts.structure_seconds + facts.weights_seconds,
        solve_seconds: facts.solve_seconds,
        requested_preconditioner: if lsmr {
            attempt.requested_preconditioner.or(used_preconditioner)
        } else {
            None
        },
        used_preconditioner,
        ..SolveReport::single_route(
            if lsmr {
                FeSolverKind::Lsmr
            } else {
                FeSolverKind::Cg
            },
            SolveStopReason::Certified,
            FeCertificate {
                absolute_infinity_norm: certificate.absolute_infinity_norm,
                relative_infinity_norm: certificate.relative_infinity_norm,
                relative_two_norm: certificate.relative_two_norm,
                passed: certificate.passed,
            },
        )
    };
    solve_report.set_route(route(&report.route));
    Ok(solve_report)
}

/// One trace line per projection (solver trace only): the outcome, work and
/// preconditioner, including the levels' channel blocks found singular.
fn trace(
    attempt: &Attempt,
    outcome: &str,
    facts: &fedemean::Facts,
    route: Option<&fedemean::Route>,
) {
    if !crate::diagnostics::solver_trace_enabled() {
        return;
    }
    let preconditioner = match route.map(|r| r.preconditioner) {
        Some(fedemean::PreconditionerUsed::ChannelBlocks { singular }) => {
            format!("channel_blocks:singular={singular}")
        }
        Some(fedemean::PreconditionerUsed::Schwarz { subdomains, local }) => format!(
            "schwarz:{subdomains}:trivial={},dense={},skyline={},approximate={}",
            local.trivial, local.dense, local.skyline, local.approximate
        ),
        Some(fedemean::PreconditionerUsed::Identity) => "none".to_owned(),
        Some(_) => "jacobi".to_owned(),
        None => "unknown".to_owned(),
    };
    crate::diagnostics::emit_solver_trace(&format!(
        "FEREG_SOLVER_TRACE|phase=fedemean_general|outcome={outcome}|method={:?}|tolerance={:.3e}|iterations={}|escalations={}|preconditioner={preconditioner}|solve_us={}",
        attempt.choice.method,
        attempt.tolerance,
        facts.iterations,
        facts.escalations,
        (facts.solve_seconds * 1e6) as u64,
    ));
}

#[cfg(test)]
mod tests;
