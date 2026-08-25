use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::ProjectionWorkspace;
use crate::fe::{
    certify_residuals_with_reference_norms, project_effect_in_place, weighted_column_norms,
    FeDesign, FeSolverKind, SolveReport, SolveStopReason,
};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapConfig {
    pub tolerance: f64,
    pub max_iterations: usize,
    pub check_every: usize,
    pub symmetric: bool,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 16_000,
            check_every: 4,
            symmetric: true,
        }
    }
}

/// Residualize one or more right-hand sides with weighted symmetric MAP.
pub fn absorb_map(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: MapConfig,
) -> Result<SolveReport> {
    validate_config(config)?;
    let started = Instant::now();
    if design.nobs() != values.nrows() || weights.len() != values.nrows() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "MAP inputs have inconsistent observation counts",
        ));
    }
    let reference_norms = weighted_column_norms(values, weights)?;
    if design.effects().is_empty() {
        let certificate = certify_residuals_with_reference_norms(
            design,
            weights,
            values,
            &reference_norms,
            config.tolerance,
        )?;
        return Ok(SolveReport {
            requested_solver: FeSolverKind::Map,
            used_solver: FeSolverKind::Map,
            iterations: 0,
            stop_reason: SolveStopReason::NoFixedEffects,
            certificate,
            normal_equation_residual: 0.0,
            setup_seconds: 0.0,
            solve_seconds: started.elapsed().as_secs_f64(),
            fallback_used: false,
            requested_preconditioner: None,
            used_preconditioner: None,
            pilot_iterations: 0,
            pilot_contraction: 0.0,
            warnings: Vec::new(),
        });
    }
    let mut workspace = ProjectionWorkspace::new(design.max_channels(), values.ncols())?;
    let mut final_certificate = certify_residuals_with_reference_norms(
        design,
        weights,
        values,
        &reference_norms,
        config.tolerance,
    )?;

    for iteration in 1..=config.max_iterations {
        for effect in design.effects() {
            project_effect_in_place(effect, weights, values, &mut workspace)?;
        }
        if config.symmetric && design.effects().len() > 1 {
            for effect in design.effects().iter().rev().skip(1) {
                project_effect_in_place(effect, weights, values, &mut workspace)?;
            }
        }
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            final_certificate = certify_residuals_with_reference_norms(
                design,
                weights,
                values,
                &reference_norms,
                config.tolerance,
            )?;
            if final_certificate.passed {
                return Ok(SolveReport {
                    requested_solver: FeSolverKind::Map,
                    used_solver: FeSolverKind::Map,
                    iterations: iteration,
                    stop_reason: SolveStopReason::Certified,
                    normal_equation_residual: final_certificate.relative_two_norm,
                    certificate: final_certificate,
                    setup_seconds: 0.0,
                    solve_seconds: started.elapsed().as_secs_f64(),
                    fallback_used: false,
                    requested_preconditioner: None,
                    used_preconditioner: None,
                    pilot_iterations: 0,
                    pilot_contraction: 0.0,
                    warnings: Vec::new(),
                });
            }
        }
    }

    Ok(SolveReport {
        requested_solver: FeSolverKind::Map,
        used_solver: FeSolverKind::Map,
        iterations: config.max_iterations,
        stop_reason: SolveStopReason::MaximumIterations,
        normal_equation_residual: final_certificate.relative_two_norm,
        certificate: final_certificate,
        setup_seconds: 0.0,
        solve_seconds: started.elapsed().as_secs_f64(),
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
    })
}

fn validate_config(config: MapConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "MAP tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 || config.check_every == 0 {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "MAP iteration and check budgets must be positive",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn absorbs_two_way_intercepts_and_certifies() {
        let worker = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("worker");
        let firm = EffectTerm::new(vec![0, 1, 0, 1, 0, 1], 2, true, vec![]).expect("firm");
        let design = FeDesign::new(6, vec![worker, firm]).expect("design");
        let weights = vec![1.0; 6];
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 2.0, 3.0, 4.0, 5.0]]).expect("values");
        let report =
            absorb_map(&design, &weights, &mut values, MapConfig::default()).expect("absorb");
        assert_eq!(report.stop_reason, SolveStopReason::Certified);
        assert!(report.certificate.passed);
        assert!(values.as_slice().iter().all(|value| value.abs() < 1e-8));
    }
}
