use std::fs::OpenOptions;
use std::io::Write;

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    certify_residuals_with_reference_norms, weighted_column_norms, FeDesign, FeSolverKind,
    LsmrPreconditioner, SolveReport, SolveStopReason,
};
use crate::options::ThreadConfig;
use within::{Effect, LsmrOptions, PreconditionerConfig};

fn emit_solver_trace(line: &str) {
    eprintln!("{line}");
    let Some(path) = std::env::var_os("FEREG_SOLVER_TRACE_FILE") else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
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
            checkpoint_retries: 0,
            solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
            requested_solver: FeSolverKind::Lsmr,
            used_solver: FeSolverKind::Lsmr,
            iterations: 0,
            stop_reason: SolveStopReason::NoFixedEffects,
            certificate,
            normal_equation_residual: 0.0,
            setup_seconds: 0.0,
            solve_seconds: 0.0,
            certification_seconds: 0.0,
            fallback_used: false,
            requested_preconditioner: Some(config.preconditioner),
            used_preconditioner: Some(resolve_preconditioner_kind(design, config.preconditioner)),
            pilot_iterations: 0,
            pilot_contraction: 0.0,
            warnings: Vec::new(),
        });
    }

    let effects = build_effects(design)?;
    let mut rhs = reserved_empty(values.ncols(), "unable to reserve LSMR right-hand sides")?;
    for column in 0..values.ncols() {
        rhs.push(values.column(column)?);
    }
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
    );
    let (solved, preconditioner_fallback) = match initial {
        Ok(solved) => (solved, false),
        Err(error)
            if config.preconditioner == LsmrPreconditioner::Auto
                && error.preconditioner_breakdown() =>
        {
            let solved = super::lsmr_batch::solve_batch(
                build_effects(design)?,
                &rhs_views,
                Some(weights),
                &options,
                PreconditionerConfig::Off,
            )
            .map_err(|fallback_error| {
                fallback_error.into_fereg("modified LSMR failed after unpreconditioned fallback")
            })?;
            (solved, true)
        }
        Err(error) => {
            return Err(error.into_fereg("modified LSMR failed"));
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
    if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
        for column in 0..values.ncols() {
            let one_column = DenseMatrix::from_columns(&[values.column(column)?])?;
            let column_certificate = certify_residuals_with_reference_norms(
                design,
                weights,
                &one_column,
                &reference_norms[column..=column],
                config.tolerance,
            )?;
            emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=lsmr_rhs|rhs={column}|converged={}|iterations={}|within_normal={:.17e}|fereg_relinf={:.17e}",
                solved.converged[column],
                solved.iterations[column],
                solved.residual[column],
                column_certificate.relative_infinity_norm,
            ));
        }
    }
    let recurrence_converged = solved.converged.iter().copied().all(|value| value);
    let stop_reason = if certificate.passed {
        SolveStopReason::Certified
    } else if recurrence_converged {
        SolveStopReason::Uncertified
    } else {
        SolveStopReason::MaximumIterations
    };
    let warning_capacity = solved
        .warnings
        .len()
        .checked_add(usize::from(preconditioner_fallback))
        .and_then(|count| {
            count.checked_add(usize::from(recurrence_converged && !certificate.passed))
        })
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
    if recurrence_converged && !certificate.passed {
        warnings.push(
            "LSMR recurrence claimed convergence but the recomputed FE certificate failed"
                .to_string(),
        );
    }

    Ok(SolveReport {
        checkpoint_retries: 0,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
        requested_solver: FeSolverKind::Lsmr,
        used_solver: FeSolverKind::Lsmr,
        iterations: solved.iterations.iter().copied().max().unwrap_or(0),
        stop_reason,
        certificate,
        normal_equation_residual: solved.residual.iter().copied().fold(0.0_f64, f64::max),
        setup_seconds: solved.time_setup,
        solve_seconds: solved.time_solve.iter().copied().sum(),
        certification_seconds: 0.0,
        fallback_used: preconditioner_fallback,
        requested_preconditioner: Some(config.preconditioner),
        used_preconditioner: Some(if preconditioner_fallback {
            LsmrPreconditioner::Off
        } else {
            resolved_preconditioner
        }),
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings,
    })
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
        LsmrPreconditioner::Schwarz => PreconditionerConfig::default(),
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
