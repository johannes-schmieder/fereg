use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    certify_residuals_with_reference_norms, project_effect_in_place, weighted_column_norms,
    FeDesign, FeSolverKind, MapConfig, ProjectionWorkspace, SolveReport, SolveStopReason,
};
use std::time::Instant;

/// Residualize with a safeguarded acceleration of symmetric MAP.
///
/// Every extrapolated candidate is checked with the independent FE-moment
/// certificate and is retained only when it improves on two ordinary sweeps.
/// Rejected steps therefore reduce exactly to certified MAP progress.
pub fn absorb_accelerated_map(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: MapConfig,
    solver: FeSolverKind,
) -> Result<SolveReport> {
    if !matches!(
        solver,
        FeSolverKind::Cg | FeSolverKind::IronsTuck | FeSolverKind::Squarem
    ) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "accelerated MAP requires cg, irons-tuck, or squarem",
        ));
    }
    if !config.tolerance.is_finite()
        || config.tolerance <= 0.0
        || config.max_iterations == 0
        || config.check_every == 0
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "accelerated MAP configuration is invalid",
        ));
    }
    if design.nobs() != values.nrows() || weights.len() != values.nrows() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "accelerated MAP inputs have inconsistent observation counts",
        ));
    }

    let started = Instant::now();
    let reference_norms = weighted_column_norms(values, weights)?;
    let mut certificate = certify_residuals_with_reference_norms(
        design,
        weights,
        values,
        &reference_norms,
        config.tolerance,
    )?;
    if design.effects().is_empty() || certificate.passed {
        return Ok(report(
            solver,
            0,
            if design.effects().is_empty() {
                SolveStopReason::NoFixedEffects
            } else {
                SolveStopReason::Certified
            },
            certificate,
            started.elapsed().as_secs_f64(),
            0,
        ));
    }

    let mut workspace = ProjectionWorkspace::new(design.max_channels(), values.ncols())?;
    let mut iterations = 0;
    let mut accepted = 0;
    while iterations < config.max_iterations {
        let x0 = values.clone();
        sweep(design, weights, values, &mut workspace, config.symmetric)?;
        iterations += 1;
        certificate = certify_residuals_with_reference_norms(
            design,
            weights,
            values,
            &reference_norms,
            config.tolerance,
        )?;
        if certificate.passed {
            return Ok(report(
                solver,
                iterations,
                SolveStopReason::Certified,
                certificate,
                started.elapsed().as_secs_f64(),
                accepted,
            ));
        }
        if iterations == config.max_iterations {
            break;
        }
        let x1 = values.clone();
        sweep(design, weights, values, &mut workspace, config.symmetric)?;
        iterations += 1;
        let baseline_certificate = certify_residuals_with_reference_norms(
            design,
            weights,
            values,
            &reference_norms,
            config.tolerance,
        )?;
        if baseline_certificate.passed {
            return Ok(report(
                solver,
                iterations,
                SolveStopReason::Certified,
                baseline_certificate,
                started.elapsed().as_secs_f64(),
                accepted,
            ));
        }

        let x2 = values.clone();
        let candidate = extrapolate(&x0, &x1, &x2, solver)?;
        if let Some(candidate) = candidate {
            let candidate_certificate = certify_residuals_with_reference_norms(
                design,
                weights,
                &candidate,
                &reference_norms,
                config.tolerance,
            )?;
            if candidate_certificate.relative_two_norm < baseline_certificate.relative_two_norm {
                *values = candidate;
                certificate = candidate_certificate;
                accepted += 1;
                if certificate.passed {
                    return Ok(report(
                        solver,
                        iterations,
                        SolveStopReason::Certified,
                        certificate,
                        started.elapsed().as_secs_f64(),
                        accepted,
                    ));
                }
                continue;
            }
        }
        certificate = baseline_certificate;
    }

    Ok(report(
        solver,
        iterations,
        SolveStopReason::MaximumIterations,
        certificate,
        started.elapsed().as_secs_f64(),
        accepted,
    ))
}

fn sweep(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    workspace: &mut ProjectionWorkspace,
    symmetric: bool,
) -> Result<()> {
    for effect in design.effects() {
        project_effect_in_place(effect, weights, values, workspace)?;
    }
    if symmetric && design.effects().len() > 1 {
        for effect in design.effects().iter().rev().skip(1) {
            project_effect_in_place(effect, weights, values, workspace)?;
        }
    }
    Ok(())
}

fn extrapolate(
    x0: &DenseMatrix,
    x1: &DenseMatrix,
    x2: &DenseMatrix,
    solver: FeSolverKind,
) -> Result<Option<DenseMatrix>> {
    let mut d1d1 = 0.0;
    let mut d1d2 = 0.0;
    let mut d2d2 = 0.0;
    for ((a, b), c) in x0.as_slice().iter().zip(x1.as_slice()).zip(x2.as_slice()) {
        let d1 = b - a;
        let d2 = c - 2.0 * b + a;
        d1d1 = d1.mul_add(d1, d1d1);
        d1d2 = d1.mul_add(d2, d1d2);
        d2d2 = d2.mul_add(d2, d2d2);
    }
    if !d2d2.is_finite() || d2d2 <= f64::EPSILON * d1d1.max(1.0) {
        return Ok(None);
    }
    let alpha = match solver {
        FeSolverKind::Cg => (-d1d2 / d2d2).clamp(-4.0, 4.0),
        FeSolverKind::IronsTuck => (-d1d2 / d2d2).clamp(-8.0, 8.0),
        FeSolverKind::Squarem => (-(d1d1 / d2d2).sqrt()).clamp(-8.0, -0.1),
        _ => unreachable!("validated accelerator"),
    };
    if !alpha.is_finite() {
        return Ok(None);
    }
    let mut candidate = x0.clone();
    for (((output, a), b), c) in candidate
        .as_mut_slice()
        .iter_mut()
        .zip(x0.as_slice())
        .zip(x1.as_slice())
        .zip(x2.as_slice())
    {
        let d1 = b - a;
        let d2 = c - 2.0 * b + a;
        *output = match solver {
            FeSolverKind::Cg => b + alpha * d1,
            FeSolverKind::IronsTuck | FeSolverKind::Squarem => {
                a + 2.0 * alpha * d1 + alpha * alpha * d2
            }
            _ => unreachable!("validated accelerator"),
        };
        if !output.is_finite() {
            return Ok(None);
        }
    }
    Ok(Some(candidate))
}

fn report(
    solver: FeSolverKind,
    iterations: usize,
    stop_reason: SolveStopReason,
    certificate: crate::fe::FeCertificate,
    solve_seconds: f64,
    accepted: usize,
) -> SolveReport {
    SolveReport {
        requested_solver: solver,
        used_solver: solver,
        iterations,
        stop_reason,
        normal_equation_residual: certificate.relative_two_norm,
        certificate,
        setup_seconds: 0.0,
        solve_seconds,
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: if accepted == 0 {
            vec!["accelerator safeguards retained ordinary MAP sweeps".to_owned()]
        } else {
            vec![format!("accepted {accepted} certified extrapolation steps")]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn every_accelerator_matches_the_certified_map_complement() {
        let a = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("a");
        let b = EffectTerm::new(vec![0, 1, 1, 2, 2, 0], 3, true, vec![]).expect("b");
        let design = FeDesign::new(6, vec![a, b]).expect("design");
        for solver in [
            FeSolverKind::Cg,
            FeSolverKind::IronsTuck,
            FeSolverKind::Squarem,
        ] {
            let mut values =
                DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0]]).expect("values");
            let report = absorb_accelerated_map(
                &design,
                &[1.0; 6],
                &mut values,
                MapConfig::default(),
                solver,
            )
            .expect("accelerated map");
            assert!(report.certified(), "{solver:?}: {report:?}");
            assert_eq!(report.used_solver, solver);
        }
    }
}
