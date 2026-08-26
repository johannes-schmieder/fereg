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

    if solver == FeSolverKind::Cg {
        return absorb_conjugate_gradient(design, weights, values, config);
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
        let x0 = fallible_matrix_copy(values, "unable to reserve accelerator x0")?;
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
        let x1 = fallible_matrix_copy(values, "unable to reserve accelerator x1")?;
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

        let x2 = fallible_matrix_copy(values, "unable to reserve accelerator x2")?;
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

/// Residualize with genuine batched weighted conjugate gradients over the
/// symmetric-Kaczmarz projection operator.
///
/// If `M` is one complete symmetric residual sweep, the positive
/// semidefinite operator used by CG is `Pi(x) = x - M(x)`.  Each right-hand
/// side has its own weighted alpha and Fletcher--Reeves beta.  Recurrence
/// progress only controls when the independent fixed-effect certificate is
/// recomputed; it is never accepted as the scientific stopping condition.
fn absorb_conjugate_gradient(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: MapConfig,
) -> Result<SolveReport> {
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
            FeSolverKind::Cg,
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
    let mut residual = apply_symmetric_projection(design, weights, values, &mut workspace)?;
    let mut direction = fallible_matrix_copy(&residual, "unable to reserve CG direction")?;
    let mut residual_square = weighted_dot_columns(&residual, &residual, weights)?;
    let initial_residual_square = fallible_vector_copy(
        &residual_square,
        "unable to reserve CG initial residual norms",
    )?;
    let mut recurrence_residual =
        largest_relative_residual(&residual_square, &initial_residual_square)?;

    for iteration in 1..=config.max_iterations {
        let projection = apply_symmetric_projection(design, weights, &direction, &mut workspace)?;
        let direction_projection = weighted_dot_columns(&direction, &projection, weights)?;
        let alpha = match cg_ratios(
            &residual_square,
            &direction_projection,
            "CG direction denominator is nonfinite or nonpositive",
        ) {
            Ok(alpha) => alpha,
            Err(error) => {
                // A semidefinite projection operator can lose its remaining
                // direction to roundoff exactly at convergence.  Accept that
                // boundary only when the independent FE certificate passes.
                certificate = certify_residuals_with_reference_norms(
                    design,
                    weights,
                    values,
                    &reference_norms,
                    config.tolerance,
                )?;
                if certificate.passed {
                    return Ok(cg_report(
                        iteration - 1,
                        SolveStopReason::Certified,
                        certificate,
                        recurrence_residual,
                        started.elapsed().as_secs_f64(),
                    ));
                }
                return Err(error);
            }
        };

        update_solution_and_residual(values, &mut residual, &direction, &projection, &alpha)?;
        let next_residual_square = weighted_dot_columns(&residual, &residual, weights)?;
        recurrence_residual =
            largest_relative_residual(&next_residual_square, &initial_residual_square)?;

        if iteration % config.check_every == 0
            || recurrence_residual <= config.tolerance.sqrt()
            || iteration == config.max_iterations
        {
            certificate = certify_residuals_with_reference_norms(
                design,
                weights,
                values,
                &reference_norms,
                config.tolerance,
            )?;
            if certificate.passed {
                return Ok(cg_report(
                    iteration,
                    SolveStopReason::Certified,
                    certificate,
                    recurrence_residual,
                    started.elapsed().as_secs_f64(),
                ));
            }
        }

        let beta = cg_ratios(
            &next_residual_square,
            &residual_square,
            "CG residual denominator is nonfinite or nonpositive",
        )?;
        update_direction(&mut direction, &residual, &beta)?;
        residual_square = next_residual_square;
    }

    Ok(cg_report(
        config.max_iterations,
        SolveStopReason::MaximumIterations,
        certificate,
        recurrence_residual,
        started.elapsed().as_secs_f64(),
    ))
}

fn apply_symmetric_projection(
    design: &FeDesign,
    weights: &[f64],
    input: &DenseMatrix,
    workspace: &mut ProjectionWorkspace,
) -> Result<DenseMatrix> {
    let mut residual = fallible_matrix_copy(input, "unable to reserve CG projection matrix")?;
    sweep(design, weights, &mut residual, workspace, true)?;
    for (output, original) in residual.as_mut_slice().iter_mut().zip(input.as_slice()) {
        *output = *original - *output;
        if !output.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "CG symmetric projection produced a nonfinite value",
            ));
        }
    }
    Ok(residual)
}

fn weighted_dot_columns(
    left: &DenseMatrix,
    right: &DenseMatrix,
    weights: &[f64],
) -> Result<Vec<f64>> {
    if left.nrows() != right.nrows()
        || left.ncols() != right.ncols()
        || weights.len() != left.nrows()
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "CG weighted-dot dimensions differ",
        ));
    }
    let weight_scale = weights.iter().copied().fold(0.0_f64, f64::max);
    if !weight_scale.is_finite()
        || weight_scale <= 0.0
        || weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0 || *weight / weight_scale == 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "CG weights must be finite, positive, and safely normalizable",
        ));
    }
    let mut output = fallible_zeros(left.ncols(), "unable to reserve CG weighted dots")?;
    let mut corrections = fallible_zeros(
        left.ncols(),
        "unable to reserve CG weighted-dot corrections",
    )?;
    for row in 0..left.nrows() {
        let weight = weights[row] / weight_scale;
        for rhs in 0..left.ncols() {
            let term = weight * left[(row, rhs)] * right[(row, rhs)];
            if !term.is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "CG weighted inner product produced a nonfinite term",
                ));
            }
            let updated = output[rhs] + term;
            corrections[rhs] += if output[rhs].abs() >= term.abs() {
                (output[rhs] - updated) + term
            } else {
                (term - updated) + output[rhs]
            };
            output[rhs] = updated;
        }
    }
    for rhs in 0..output.len() {
        output[rhs] += corrections[rhs];
        if !output[rhs].is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "CG weighted inner product is nonfinite",
            ));
        }
    }
    Ok(output)
}

fn cg_ratios(numerators: &[f64], denominators: &[f64], message: &'static str) -> Result<Vec<f64>> {
    if numerators.len() != denominators.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "CG recurrence dimensions differ",
        ));
    }
    let mut ratios = Vec::new();
    ratios.try_reserve_exact(numerators.len()).map_err(|_| {
        FeregError::new(
            ErrorCode::Allocation,
            "unable to reserve CG recurrence ratios",
        )
    })?;
    for (numerator, denominator) in numerators.iter().zip(denominators) {
        if !numerator.is_finite() || *numerator < 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "CG recurrence numerator is invalid",
            ));
        }
        if *numerator == 0.0 {
            ratios.push(0.0);
            continue;
        }
        if !denominator.is_finite() || *denominator <= 0.0 {
            return Err(FeregError::new(ErrorCode::InvalidData, message));
        }
        let ratio = numerator / denominator;
        if !ratio.is_finite() || ratio < 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "CG recurrence ratio is invalid",
            ));
        }
        ratios.push(ratio);
    }
    Ok(ratios)
}

fn update_solution_and_residual(
    values: &mut DenseMatrix,
    residual: &mut DenseMatrix,
    direction: &DenseMatrix,
    projection: &DenseMatrix,
    alpha: &[f64],
) -> Result<()> {
    for row in 0..values.nrows() {
        for rhs in 0..values.ncols() {
            values[(row, rhs)] -= alpha[rhs] * direction[(row, rhs)];
            residual[(row, rhs)] -= alpha[rhs] * projection[(row, rhs)];
            if !values[(row, rhs)].is_finite() || !residual[(row, rhs)].is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "CG update produced a nonfinite value",
                ));
            }
        }
    }
    Ok(())
}

fn update_direction(
    direction: &mut DenseMatrix,
    residual: &DenseMatrix,
    beta: &[f64],
) -> Result<()> {
    for row in 0..direction.nrows() {
        for rhs in 0..direction.ncols() {
            direction[(row, rhs)] = residual[(row, rhs)] + beta[rhs] * direction[(row, rhs)];
            if !direction[(row, rhs)].is_finite() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    "CG direction update produced a nonfinite value",
                ));
            }
        }
    }
    Ok(())
}

fn largest_relative_residual(current: &[f64], initial: &[f64]) -> Result<f64> {
    if current.len() != initial.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "CG residual-norm dimensions differ",
        ));
    }
    let mut largest = 0.0_f64;
    for (current, initial) in current.iter().zip(initial) {
        if !current.is_finite() || *current < 0.0 || !initial.is_finite() || *initial < 0.0 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "CG residual norm is invalid",
            ));
        }
        let relative = if *initial == 0.0 {
            if *current == 0.0 {
                0.0
            } else {
                f64::INFINITY
            }
        } else {
            (current / initial).sqrt()
        };
        if !relative.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "CG relative residual is nonfinite",
            ));
        }
        largest = largest.max(relative);
    }
    Ok(largest)
}

fn cg_report(
    iterations: usize,
    stop_reason: SolveStopReason,
    certificate: crate::fe::FeCertificate,
    recurrence_residual: f64,
    solve_seconds: f64,
) -> SolveReport {
    SolveReport {
        requested_solver: FeSolverKind::Cg,
        used_solver: FeSolverKind::Cg,
        iterations,
        stop_reason,
        certificate,
        normal_equation_residual: recurrence_residual,
        setup_seconds: 0.0,
        solve_seconds,
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
    }
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
    let mut candidate = fallible_matrix_copy(x0, "unable to reserve extrapolated candidate")?;
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

fn fallible_matrix_copy(matrix: &DenseMatrix, message: &'static str) -> Result<DenseMatrix> {
    let values = fallible_vector_copy(matrix.as_slice(), message)?;
    DenseMatrix::from_row_major(matrix.nrows(), matrix.ncols(), values)
}

fn fallible_vector_copy<T: Copy>(values: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    output.extend_from_slice(values);
    Ok(output)
}

fn fallible_zeros(elements: usize, message: &'static str) -> Result<Vec<f64>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, message))?;
    output.resize(elements, 0.0);
    Ok(output)
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
