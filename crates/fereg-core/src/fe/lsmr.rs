use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    certify_residuals_with_reference_norms, weighted_column_norms, FeDesign, FeSolverKind,
    LsmrPreconditioner, SolveReport, SolveStopReason,
};
use crate::options::ThreadConfig;
use within::{Effect, LsmrOptions, PreconditionerConfig};

/// Modified-LSMR tuning for fixed-effect absorption.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LsmrConfig {
    pub tolerance: f64,
    pub max_iterations: usize,
    /// Windowed modified Gram-Schmidt history. `None` disables reorthogonalization.
    pub local_size: Option<usize>,
    pub preconditioner: LsmrPreconditioner,
    pub threads: ThreadConfig,
}

impl Default for LsmrConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 16_000,
            local_size: Some(4),
            preconditioner: LsmrPreconditioner::Auto,
            threads: ThreadConfig::default(),
        }
    }
}

fn reserved_empty<T>(elements: usize, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    Ok(output)
}

fn build_effects(design: &FeDesign) -> Result<Vec<Effect<'_>>> {
    let mut effects = reserved_empty(
        design.effects().len(),
        "unable to reserve LSMR fixed-effect views",
    )?;
    for effect in design.effects() {
        effects.push(
            Effect::new(
                effect.codes(),
                effect.has_intercept(),
                effect.slopes().iter().map(Vec::as_slice),
            )
            .map_err(|error| {
                FeregError::new(
                    ErrorCode::InvalidData,
                    format!("unable to construct LSMR fixed-effect term: {error}"),
                )
            })?,
        );
    }
    Ok(effects)
}

/// Residualize one or more right-hand sides with the pinned `within` modified-LSMR core.
///
/// All input materialization and all output writes occur on the caller thread.
/// Numerical work uses the caller's current Rayon pool, which is the isolated
/// action-local `ThreadTeam` in public estimation. It never has access to
/// Stata's SPI.
pub fn absorb_lsmr(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: LsmrConfig,
) -> Result<SolveReport> {
    absorb_lsmr_certified_at(design, weights, values, config, config.tolerance)
}

/// [`absorb_lsmr`] with the recurrence stopping at `config.tolerance` and the
/// independent FE certificate (and the stop reason it decides) at
/// `certificate_tolerance`.
pub(crate) fn absorb_lsmr_certified_at(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: LsmrConfig,
    certificate_tolerance: f64,
) -> Result<SolveReport> {
    crate::interrupt::checkpoint()?;
    validate_config(config)?;
    if !certificate_tolerance.is_finite() || certificate_tolerance <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR tolerance must be finite and positive",
        ));
    }
    if design.nobs() != values.nrows() || weights.len() != values.nrows() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR inputs have inconsistent observation counts",
        ));
    }
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "LSMR weights must be finite and positive",
        ));
    }
    let reference_norms = weighted_column_norms(values, weights)?;
    if design.effects().is_empty() || values.ncols() == 0 {
        let certificate = certify_residuals_with_reference_norms(
            design,
            weights,
            values,
            &reference_norms,
            certificate_tolerance,
        )?;
        return Ok(SolveReport {
            requested_preconditioner: Some(config.preconditioner),
            used_preconditioner: Some(resolve_preconditioner_kind(design, config.preconditioner)),
            ..SolveReport::single_route(
                FeSolverKind::Lsmr,
                SolveStopReason::NoFixedEffects,
                certificate,
            )
        });
    }

    let effects = build_effects(design)?;
    let rhs = super::lsmr_batch::rhs_columns(values)?;
    let mut rhs_views = reserved_empty(
        rhs.len(),
        "unable to reserve borrowed LSMR right-hand-side views",
    )?;
    for column in &rhs {
        rhs_views.push(column.as_slice());
    }
    let options = LsmrOptions {
        tol: config.tolerance,
        maxiter: config.max_iterations,
        local_size: config.local_size,
    };
    let resolved_preconditioner = resolve_preconditioner_kind(design, config.preconditioner);
    let preconditioner = within_preconditioner(resolved_preconditioner);
    let initial = super::lsmr_batch::solve_batch(
        effects,
        &rhs_views,
        Some(weights),
        &options,
        preconditioner,
        config.threads.deterministic,
    );
    let (solved, preconditioner_fallback) = match initial {
        Ok(solved) => (solved, false),
        Err(error)
            if config.preconditioner == LsmrPreconditioner::Auto
                && error.preconditioner_breakdown() =>
        {
            let solved = retry_owned_unpreconditioned(
                design,
                weights,
                &rhs_views,
                &options,
                config.threads.deterministic,
                error,
            )?;
            (solved, true)
        }
        Err(error) => {
            return Err(error.into_fereg("modified LSMR failed"));
        }
    };

    super::lsmr_batch::copy_demeaned(&solved.demeaned, values);
    let certificate = certify_residuals_with_reference_norms(
        design,
        weights,
        values,
        &reference_norms,
        certificate_tolerance,
    )?;
    if crate::diagnostics::solver_trace_enabled() {
        trace_lsmr_columns(
            design,
            weights,
            values,
            &reference_norms,
            certificate_tolerance,
            &solved,
        )?;
    }
    let recurrence_converged = solved.converged.iter().copied().all(|value| value);
    let stop_reason = if certificate.passed {
        SolveStopReason::Certified
    } else if recurrence_converged {
        SolveStopReason::Uncertified
    } else {
        SolveStopReason::MaximumIterations
    };
    let warnings = lsmr_warnings(
        &solved,
        preconditioner_fallback,
        recurrence_converged && !certificate.passed,
    )?;

    Ok(SolveReport {
        iterations: solved.iterations.iter().copied().max().unwrap_or(0),
        normal_equation_residual: solved.residual.iter().copied().fold(0.0_f64, f64::max),
        setup_seconds: solved.time_setup,
        solve_seconds: solved.time_solve.iter().copied().sum(),
        fallback_used: preconditioner_fallback,
        requested_preconditioner: Some(config.preconditioner),
        used_preconditioner: Some(if preconditioner_fallback {
            LsmrPreconditioner::Off
        } else {
            resolved_preconditioner
        }),
        warnings,
        ..SolveReport::single_route(FeSolverKind::Lsmr, stop_reason, certificate)
    })
}

// A numerical refusal consumes its attempted native work before the only
// permitted Auto retry. Explicit preconditioners never enter this helper.
fn retry_owned_unpreconditioned(
    design: &FeDesign,
    weights: &[f64],
    rhs: &[&[f64]],
    options: &LsmrOptions,
    deterministic: bool,
    error: super::lsmr_batch::BatchError,
) -> Result<within::BatchSolveResult> {
    let spent = error.attempted_iterations();
    let Some(remaining) = options.maxiter.checked_sub(spent).filter(|n| *n > 0) else {
        return Err(error.into_fereg("modified LSMR failed"));
    };
    let fallback_options = LsmrOptions {
        maxiter: remaining,
        ..options.clone()
    };
    let mut solved = super::lsmr_batch::solve_batch(
        build_effects(design)?,
        rhs,
        Some(weights),
        &fallback_options,
        PreconditionerConfig::Off,
        deterministic,
    )
    .map_err(|fallback_error| {
        fallback_error.into_fereg("modified LSMR failed after unpreconditioned fallback")
    })?;
    for iterations in &mut solved.iterations {
        *iterations += spent;
    }
    Ok(solved)
}

fn trace_lsmr_columns(
    design: &FeDesign,
    weights: &[f64],
    values: &DenseMatrix,
    reference_norms: &[f64],
    certificate_tolerance: f64,
    solved: &within::BatchSolveResult,
) -> Result<()> {
    for column in 0..values.ncols() {
        let one_column = DenseMatrix::from_columns(&[values.column(column)?])?;
        let column_certificate = certify_residuals_with_reference_norms(
            design,
            weights,
            &one_column,
            &reference_norms[column..=column],
            certificate_tolerance,
        )?;
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=lsmr_rhs|rhs={column}|converged={}|iterations={}|within_normal={:.17e}|fereg_relinf={:.17e}",
            solved.converged[column],
            solved.iterations[column],
            solved.residual[column],
            column_certificate.relative_infinity_norm,
        ));
    }
    Ok(())
}

fn lsmr_warnings(
    solved: &within::BatchSolveResult,
    preconditioner_fallback: bool,
    converged_but_uncertified: bool,
) -> Result<Vec<String>> {
    let warning_capacity = solved
        .warnings
        .len()
        .checked_add(usize::from(preconditioner_fallback))
        .and_then(|count| count.checked_add(usize::from(converged_but_uncertified)))
        .ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "LSMR warning count overflows usize")
        })?;
    let mut warnings = reserved_empty(warning_capacity, "unable to reserve LSMR warnings")?;
    for warning in &solved.warnings {
        warnings.push(format!("within preconditioner: {warning:?}"));
    }
    if preconditioner_fallback {
        warnings.insert(
            0,
            "automatic LSMR preconditioner failed its positive-definiteness guard; retried unpreconditioned and certified the returned residuals".to_owned(),
        );
    }
    if converged_but_uncertified {
        warnings.push(
            "LSMR recurrence claimed convergence but the recomputed FE certificate failed"
                .to_string(),
        );
    }
    Ok(warnings)
}

pub(crate) fn resolve_preconditioner_kind(
    design: &FeDesign,
    requested: LsmrPreconditioner,
) -> LsmrPreconditioner {
    match requested {
        LsmrPreconditioner::Off => LsmrPreconditioner::Off,
        LsmrPreconditioner::Diagonal => LsmrPreconditioner::Diagonal,
        LsmrPreconditioner::Schwarz => LsmrPreconditioner::Schwarz,
        LsmrPreconditioner::Auto => {
            if design.effects().len() >= 2 && design.total_coefficients() >= 256 {
                LsmrPreconditioner::Schwarz
            } else {
                LsmrPreconditioner::Diagonal
            }
        }
    }
}

fn within_preconditioner(preconditioner: LsmrPreconditioner) -> PreconditionerConfig {
    match preconditioner {
        LsmrPreconditioner::Auto => unreachable!("automatic preconditioner is resolved first"),
        LsmrPreconditioner::Off => PreconditionerConfig::Off,
        LsmrPreconditioner::Diagonal => PreconditionerConfig::Diagonal,
        LsmrPreconditioner::Schwarz => additive_schwarz(),
    }
}

/// Explicit additive Schwarz for fereg's own routing policy. Upstream's default
/// is adaptive; inheriting it would add a second, independent escalation policy.
/// The upstream owner reduction is repeatable at a fixed worker count.
pub(crate) fn additive_schwarz() -> PreconditionerConfig {
    PreconditionerConfig::Additive {
        local_solver: within::LocalSolverConfig::default(),
    }
}

fn validate_config(config: LsmrConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR iteration budget must be positive",
        ));
    }
    if config.local_size == Some(0) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "LSMR reorthogonalization window must be positive when supplied",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn schwarz_configuration_never_inherits_upstream_adaptive_policy() {
        assert!(matches!(
            additive_schwarz(),
            PreconditionerConfig::Additive { .. }
        ));
    }

    #[test]
    fn lsmr_setup_capacity_overflow_is_an_allocation_error() {
        let error = reserved_empty::<u8>(usize::MAX, "injected LSMR allocation")
            .expect_err("capacity overflow");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(error.return_code(), 909);
    }

    #[test]
    fn lsmr_matches_two_way_additive_projection() {
        let worker = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("worker");
        let firm = EffectTerm::new(vec![0, 1, 0, 1, 0, 1], 2, true, vec![]).expect("firm");
        let design = FeDesign::new(6, vec![worker, firm]).expect("design");
        let weights = vec![1.0; 6];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 2.0, 3.0, 4.0, 5.0]]).expect("values");
        let report = absorb_lsmr(
            &design,
            &weights,
            &mut values,
            LsmrConfig {
                preconditioner: LsmrPreconditioner::Diagonal,
                ..LsmrConfig::default()
            },
        )
        .expect("absorb");
        assert!(report.certified());
        assert!(values.as_slice().iter().all(|value| value.abs() < 1e-7));
    }

    #[test]
    fn lsmr_absorbs_group_specific_slopes() {
        let effect = EffectTerm::new(
            vec![0, 0, 0, 1, 1, 1],
            2,
            true,
            vec![vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0]],
        )
        .expect("effect");
        let design = FeDesign::new(6, vec![effect]).expect("design");
        let weights = vec![1.0, 2.0, 1.0, 1.0, 1.0, 2.0];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 3.0, 5.0, 2.0, 1.0, 0.0]]).expect("values");
        let report =
            absorb_lsmr(&design, &weights, &mut values, LsmrConfig::default()).expect("absorb");
        assert!(report.certified());
        assert!(values.as_slice().iter().all(|value| value.abs() < 1e-7));
    }

    fn extreme_weight_tree_fixture() -> (FeDesign, Vec<f64>, DenseMatrix) {
        let first = EffectTerm::new(vec![0, 0, 1, 1, 1], 2, true, vec![]).expect("first");
        let second = EffectTerm::new(vec![0, 0, 0, 1, 1], 2, true, vec![]).expect("second");
        let design = FeDesign::new(5, vec![first, second]).expect("design");
        let tiny = 1e-5_f64 * 1e-5_f64 / 3.0;
        let weights = vec![tiny, 1.0, tiny, tiny, 1.0];
        let values = DenseMatrix::from_columns(&[vec![1.0, 0.0, 1.0, 1.0, 0.0]]).expect("values");
        (design, weights, values)
    }

    #[test]
    fn extreme_weight_tree_matches_analytic_projection_with_auto_and_off() {
        // Preserve the historical roundoff fixture as a scientific witness.
        // The three observed cells form an acyclic bipartite design, so their
        // weighted means give its exact independent FE projection. Whether a
        // particular arithmetic implementation needs a positivity retry is
        // incidental; both returned projections must satisfy this certificate.
        let (design, weights, original) = extreme_weight_tree_fixture();
        let mean = weights[0] / (weights[0] + weights[1]);
        let analytic = [1.0 - mean, -mean, 0.0, 1.0 - mean, -mean];
        for preconditioner in [LsmrPreconditioner::Auto, LsmrPreconditioner::Off] {
            let mut values = original.clone();
            let report = absorb_lsmr(
                &design,
                &weights,
                &mut values,
                LsmrConfig {
                    tolerance: 1e-10,
                    preconditioner,
                    ..LsmrConfig::default()
                },
            )
            .expect("certified weighted projection");
            assert!(report.certified());
            assert!(values
                .as_slice()
                .iter()
                .zip(analytic)
                .all(|(actual, expected)| (actual - expected).abs() < 1e-14));
        }
    }

    #[test]
    fn positivity_retry_cannot_refund_the_failed_attempts_iterations() {
        let (design, _, values) = extreme_weight_tree_fixture();
        let weights = [1.0; 5];
        let rhs = super::super::lsmr_batch::rhs_columns(&values).unwrap();
        let rhs_views: Vec<_> = rhs.iter().map(Vec::as_slice).collect();
        let options = LsmrOptions {
            tol: 1e-10,
            maxiter: 500,
            local_size: Some(4),
        };
        // The upstream iteration_budget tests use genuinely failing operators
        // to establish this counted-error contract. At the adapter boundary,
        // choose the count explicitly instead of relying on accidental roundoff.
        let spent = 3;
        let failure = || super::super::lsmr_batch::BatchError::Attempt {
            source: within::SolveError::InvalidInput {
                context: "mlsmr",
                message: "preconditioner not positive definite".to_owned(),
            },
            iterations: spent,
        };
        let off = super::super::lsmr_batch::solve_batch(
            build_effects(&design).unwrap(),
            &rhs_views,
            Some(&weights),
            &options,
            PreconditionerConfig::Off,
            false,
        )
        .unwrap();
        assert!(off.converged.iter().all(|&value| value));
        let total = spent + off.iterations.iter().copied().max().unwrap();
        let successful = retry_owned_unpreconditioned(
            &design,
            &weights,
            &rhs_views,
            &LsmrOptions {
                maxiter: total,
                ..options.clone()
            },
            false,
            failure(),
        )
        .unwrap();
        assert_eq!(successful.iterations.iter().copied().max(), Some(total));
        assert_eq!(successful.demeaned, off.demeaned);
        assert_eq!(successful.x, off.x);
        let error = retry_owned_unpreconditioned(
            &design,
            &weights,
            &rhs_views,
            &LsmrOptions {
                maxiter: spent,
                ..options
            },
            false,
            failure(),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(error
            .message
            .contains("preconditioner not positive definite"));
    }
}
