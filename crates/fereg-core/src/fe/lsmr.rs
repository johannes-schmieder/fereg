use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    certify_residuals_with_reference_norms, weighted_column_norms, FeDesign, FeSolverKind,
    SolveReport, SolveStopReason,
};
use crate::options::ThreadConfig;
use rayon::ThreadPoolBuilder;
use within::{Effect, LsmrOptions, PreconditionerConfig};

/// Preconditioner exposed by the fereg modified-LSMR adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LsmrPreconditioner {
    /// Diagonal for small/simple designs, additive Schwarz otherwise.
    Auto,
    /// Identity preconditioner.
    Off,
    /// Jacobi/diagonal preconditioner.
    Diagonal,
    /// One-level additive Schwarz over factor-pair subdomains.
    Schwarz,
}

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

/// Residualize one or more right-hand sides with the pinned `within` modified-LSMR core.
///
/// All input materialization and all output writes occur on the caller thread. The
/// temporary Rayon pool encloses numerical work only; it never has access to Stata's SPI.
pub fn absorb_lsmr(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: LsmrConfig,
) -> Result<SolveReport> {
    validate_config(config)?;
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
            config.tolerance,
        )?;
        return Ok(SolveReport {
            requested_solver: FeSolverKind::Lsmr,
            used_solver: FeSolverKind::Lsmr,
            iterations: 0,
            stop_reason: SolveStopReason::NoFixedEffects,
            certificate,
            normal_equation_residual: 0.0,
            setup_seconds: 0.0,
            solve_seconds: 0.0,
            fallback_used: false,
            warnings: Vec::new(),
        });
    }

    let effects = design
        .effects()
        .iter()
        .map(|effect| {
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
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let rhs = (0..values.ncols())
        .map(|column| values.column(column))
        .collect::<Result<Vec<_>>>()?;
    let rhs_views = rhs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let options = LsmrOptions {
        tol: config.tolerance,
        maxiter: config.max_iterations,
        local_size: config.local_size,
    };
    let preconditioner = resolve_preconditioner(design, config.preconditioner);
    let pool = ThreadPoolBuilder::new()
        .num_threads(config.threads.resolved())
        .thread_name(|index| format!("fereg-lsmr-{index}"))
        .build()
        .map_err(|error| {
            FeregError::new(
                ErrorCode::Internal,
                format!("unable to build LSMR thread pool: {error}"),
            )
        })?;

    let initial = pool.install(|| {
        within::solve_batch(
            effects.clone(),
            &rhs_views,
            Some(weights),
            Some(&options),
            preconditioner,
        )
    });
    let (solved, preconditioner_fallback) = match initial {
        Ok(solved) => (solved, false),
        Err(error)
            if config.preconditioner == LsmrPreconditioner::Auto
                && is_preconditioner_positivity_breakdown(&error) =>
        {
            let solved = pool
                .install(|| {
                    within::solve_batch(
                        effects,
                        &rhs_views,
                        Some(weights),
                        Some(&options),
                        PreconditionerConfig::Off,
                    )
                })
                .map_err(|fallback_error| {
                    FeregError::new(
                        ErrorCode::Internal,
                        format!(
                            "modified LSMR failed after unpreconditioned fallback: {fallback_error}"
                        ),
                    )
                })?;
            (solved, true)
        }
        Err(error) => {
            return Err(FeregError::new(
                ErrorCode::Internal,
                format!("modified LSMR failed: {error}"),
            ));
        }
    };

    for column in 0..values.ncols() {
        let demeaned = solved.demeaned(column);
        for (row, value) in demeaned.iter().copied().enumerate() {
            values[(row, column)] = value;
        }
    }
    let certificate = certify_residuals_with_reference_norms(
        design,
        weights,
        values,
        &reference_norms,
        config.tolerance,
    )?;
    let recurrence_converged = solved.converged.iter().copied().all(|value| value);
    let stop_reason = if certificate.passed {
        SolveStopReason::Certified
    } else if recurrence_converged {
        SolveStopReason::Uncertified
    } else {
        SolveStopReason::MaximumIterations
    };
    let mut warnings = solved
        .warnings
        .iter()
        .map(|warning| format!("within preconditioner: {warning:?}"))
        .collect::<Vec<_>>();
    if preconditioner_fallback {
        warnings.insert(
            0,
            "automatic LSMR preconditioner failed its positive-definiteness guard; retried unpreconditioned and certified the returned residuals".to_owned(),
        );
    }
    if recurrence_converged && !certificate.passed {
        warnings.push(
            "LSMR recurrence claimed convergence but the recomputed FE certificate failed"
                .to_string(),
        );
    }

    Ok(SolveReport {
        requested_solver: FeSolverKind::Lsmr,
        used_solver: FeSolverKind::Lsmr,
        iterations: solved.iterations.iter().copied().max().unwrap_or(0),
        stop_reason,
        certificate,
        normal_equation_residual: solved.residual.iter().copied().fold(0.0_f64, f64::max),
        setup_seconds: solved.time_setup,
        solve_seconds: solved.time_solve.iter().copied().sum(),
        fallback_used: preconditioner_fallback,
        warnings,
    })
}

fn is_preconditioner_positivity_breakdown(error: &within::WithinError) -> bool {
    matches!(
        error,
        within::WithinError::Solve(within::SolveError::InvalidInput { context, message })
            if *context == "mlsmr" && message.contains("preconditioner not positive definite")
    )
}

fn resolve_preconditioner(
    design: &FeDesign,
    requested: LsmrPreconditioner,
) -> PreconditionerConfig {
    match requested {
        LsmrPreconditioner::Off => PreconditionerConfig::Off,
        LsmrPreconditioner::Diagonal => PreconditionerConfig::Diagonal,
        LsmrPreconditioner::Schwarz => PreconditionerConfig::default(),
        LsmrPreconditioner::Auto => {
            if design.effects().len() >= 2 && design.total_coefficients() >= 256 {
                PreconditionerConfig::default()
            } else {
                PreconditionerConfig::Diagonal
            }
        }
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

    #[test]
    fn automatic_preconditioner_falls_back_for_roundoff_indefiniteness() {
        let first = EffectTerm::new(vec![0, 0, 1, 1, 1], 2, true, vec![]).expect("first");
        let second = EffectTerm::new(vec![0, 0, 0, 1, 1], 2, true, vec![]).expect("second");
        let design = FeDesign::new(5, vec![first, second]).expect("design");
        let tiny = 1e-5_f64 * 1e-5_f64 / 3.0;
        let weights = vec![tiny, 1.0, tiny, tiny, 1.0];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 0.0, 1.0, 1.0, 0.0]]).expect("values");
        let report = absorb_lsmr(
            &design,
            &weights,
            &mut values,
            LsmrConfig {
                tolerance: 1e-10,
                ..LsmrConfig::default()
            },
        )
        .expect("certified fallback");
        assert!(report.fallback_used);
        assert!(report.certified());
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("positive-definiteness guard")));
    }
}
