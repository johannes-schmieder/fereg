//! Explicit categorical IT; automatic routing is deliberately unchanged.
//! Compact acceleration uses the Euclidean coefficient metric, changing the
//! extrapolation trajectory from the observation metric. The original-slot
//! sweep gauge and independently certified projection target are unchanged.
use super::*;

/// Compare complete simultaneously-live plans, including coefficient layout.
/// The input panel is caller-owned execution storage. A compact attempt owns
/// three coefficient banks and one residual candidate; its observation fallback
/// owns three residual snapshots and, only when requested, three coefficient
/// banks. Both use the same fully reserved sweep/certificate workspace.
fn plan(
    design: &BorrowedInterceptFeDesign<'_>,
    rhs: usize,
    retain: bool,
    budget: Option<FeSolverMemoryBudget>,
) -> Result<(bool, SolverWorkspacePeak)> {
    let overflow = || allocation("Irons-Tuck workspace dimensions overflow");
    let panel = (design.nobs() as u128)
        .checked_mul(rhs as u128)
        .and_then(|v| v.checked_mul(8))
        .ok_or_else(overflow)?;
    let coefficients = (design.total_term_slots() as u128)
        .checked_mul(rhs as u128)
        .and_then(|v| v.checked_mul(8))
        .and_then(|v| {
            v.checked_add(
                (3 * design.effects().len() as u128 + 1) * std::mem::size_of::<usize>() as u128,
            )
        })
        .ok_or_else(overflow)?;
    let shared = BorrowedMapWorkspace::planned_payload_bytes(
        design,
        design.nobs(),
        rhs,
        rayon::current_num_threads(),
    )?
    .checked_add(1024)
    .ok_or_else(overflow)?;
    let compact = coefficients
        .checked_mul(3)
        .and_then(|v| v.checked_add(panel))
        .and_then(|v| v.checked_add(shared))
        .ok_or_else(overflow)?;
    let observation = panel
        .checked_mul(3)
        .and_then(|v| {
            v.checked_add(if retain {
                coefficients.checked_mul(3)?
            } else {
                0
            })
        })
        .and_then(|v| v.checked_add(shared))
        .ok_or_else(overflow)?;
    let use_compact = compact <= observation;
    let bytes = if use_compact { compact } else { observation };
    if budget.is_some_and(|b| !b.fits(bytes)) {
        return Err(allocation("Irons-Tuck workspace exceeds usable memory"));
    }
    Ok((use_compact, SolverWorkspacePeak::from_bytes(bytes)?))
}

fn extrapolate_coefficients(
    mut first: FeCoefficientBatch,
    second: &FeCoefficientBatch,
    third: &FeCoefficientBatch,
    alpha: f64,
) -> Result<Option<FeCoefficientBatch>> {
    if !first.same_layout(second) || !first.same_layout(third) {
        return Err(invalid_argument("Irons-Tuck coefficient layouts differ"));
    }
    for (index, ((a, &b), &c)) in first
        .values
        .iter_mut()
        .zip(&second.values)
        .zip(&third.values)
        .enumerate()
    {
        crate::interrupt::row_checkpoint(index)?;
        let d1 = b - *a;
        let d2 = c - 2.0 * b + *a;
        *a = alpha.mul_add(2.0 * d1, *a).mul_add(1.0, alpha * alpha * d2);
        if !a.is_finite() {
            return Ok(None);
        }
    }
    Ok(Some(first))
}

pub(super) fn compact_candidate(
    design: &BorrowedInterceptFeDesign<'_>,
    first: FeCoefficientBatch,
    second: &FeCoefficientBatch,
    third: &FeCoefficientBatch,
    residual: &DenseMatrix,
) -> Result<Option<(DenseMatrix, Option<FeCoefficientBatch>)>> {
    if !first.same_layout(second) || !first.same_layout(third) {
        return Err(invalid_argument("Irons-Tuck coefficient layouts differ"));
    }
    let (mut d1d1, mut d1d2, mut d2d2) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (index, ((&a, &b), &c)) in first
        .values
        .iter()
        .zip(&second.values)
        .zip(&third.values)
        .enumerate()
    {
        crate::interrupt::row_checkpoint(index)?;
        let d1 = b - a;
        let d2 = c - 2.0 * b + a;
        d1d1 = d1.mul_add(d1, d1d1);
        d1d2 = d1.mul_add(d2, d1d2);
        d2d2 = d2.mul_add(d2, d2d2);
    }
    if !d2d2.is_finite() || d2d2 <= f64::EPSILON * d1d1.max(1.0) {
        return Ok(None);
    }
    let alpha = (-d1d2 / d2d2).clamp(-8.0, 8.0);
    if !alpha.is_finite() {
        return Ok(None);
    }
    let Some(coefficients) = extrapolate_coefficients(first, second, third, alpha)? else {
        return Ok(None);
    };
    let mut candidate = fallible_matrix_copy(residual, "unable to reserve compact IT candidate")?;
    // y - D*c_candidate = residual_after_two_sweeps + D*(c2-c_candidate).
    // This reconstructs only the proposal; rejecting it leaves both baseline
    // residuals and their original-slot coefficient bank untouched.
    for row in 0..design.nobs() {
        crate::interrupt::row_checkpoint(row)?;
        for rhs in 0..residual.ncols() {
            let offset = rhs * third.coefficients_per_rhs;
            let mut correction = 0.0;
            for (j, effect) in design.effects().iter().enumerate() {
                let index = offset + third.effect_offsets[j] + effect.codes()[row] as usize;
                correction += third.values[index] - coefficients.values[index];
            }
            candidate[(row, rhs)] += correction;
            if !candidate[(row, rhs)].is_finite() {
                return Ok(None);
            }
        }
    }
    Ok(Some((candidate, Some(coefficients))))
}

/// Residualize with safeguarded Irons-Tuck extrapolation while optionally
/// accelerating in compact coefficient space when its complete memory plan is
/// no larger than observation snapshots. Every proposal retains the independent
/// original-weight certificate and ordinary two-sweep rejection safeguard.
pub(crate) fn absorb_borrowed_intercept_irons_tuck_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    retain_coefficients: bool,
    budget: Option<FeSolverMemoryBudget>,
    allow_compact: bool,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config)?;
    if design.nobs() != values.nrows()
        || values.nrows() == 0
        || values.ncols() == 0
        || design.effects().is_empty()
    {
        return Err(invalid_argument(
            "borrowed Irons-Tuck inputs are empty or dimensionally inconsistent",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("borrowed Irons-Tuck values must be finite"));
    }
    let weights = validate_weights(weights, values.nrows())?;
    // The uninjected observation-metric solver retains its established admission
    // and publication behavior. Only the experimental arm adopts the new plan.
    let (compact, peak) = if allow_compact {
        plan(design, values.ncols(), retain_coefficients, budget)?
    } else {
        (false, SolverWorkspacePeak::ZERO)
    };
    let track_coefficients = compact || retain_coefficients;
    let setup_started = Instant::now();
    let mut reservation = SystemWorkspaceReservation;
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), &mut reservation)?;
    update_workspace_norms(&values, weights, &mut workspace, true)?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();
    let initial_certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
    let coefficients = track_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;
    // A passing certificate earns one confirming sweep when the budget
    // allows; the solve finishes only if the confirmed certificate passes.
    let confirm = |values: &mut DenseMatrix,
                   workspace: &mut BorrowedMapWorkspace,
                   coefficients: Option<&mut FeCoefficientBatch>,
                   iterations: &mut usize,
                   certificate: FeCertificate|
     -> Result<FeCertificate> {
        if *iterations >= config.max_iterations {
            return Ok(certificate);
        }
        borrowed_residual_sweep_recording(
            design,
            weights,
            values,
            workspace,
            coefficients,
            config.symmetric,
        )?;
        *iterations += 1;
        certify(design, weights, values, config.tolerance, workspace)
    };
    let finish = |values: DenseMatrix,
                  coefficients: Option<FeCoefficientBatch>,
                  iterations: usize,
                  certificate: FeCertificate|
     -> Result<CertifiedBorrowedMap> {
        let normal_equation_residual = certificate.relative_two_norm;
        let mut result = certified_solver_result(
            values,
            FeSolverKind::IronsTuck,
            SolveStopReason::Certified,
            iterations,
            certificate,
            normal_equation_residual,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        if allow_compact {
            result.set_solver_workspace_peak(peak);
        }
        attach_optional_coefficients(result, coefficients.filter(|_| retain_coefficients))
    };
    if initial_certificate.passed {
        let mut result = certified_solver_result(
            values,
            FeSolverKind::IronsTuck,
            SolveStopReason::Certified,
            0,
            initial_certificate,
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        if allow_compact {
            result.set_solver_workspace_peak(peak);
        }
        return attach_optional_coefficients(result, coefficients.filter(|_| retain_coefficients));
    }

    Iteration {
        design,
        weights,
        config,
        compact,
    }
    .run(values, workspace, coefficients, confirm, finish)
}

struct Iteration<'a, 'd> {
    design: &'a BorrowedInterceptFeDesign<'d>,
    weights: ValidatedWeights<'a>,
    config: MapConfig,
    compact: bool,
}

impl Iteration<'_, '_> {
    fn run(
        self,
        mut values: DenseMatrix,
        mut workspace: BorrowedMapWorkspace,
        mut coefficients: Option<FeCoefficientBatch>,
        confirm: impl Fn(
            &mut DenseMatrix,
            &mut BorrowedMapWorkspace,
            Option<&mut FeCoefficientBatch>,
            &mut usize,
            FeCertificate,
        ) -> Result<FeCertificate>,
        finish: impl Fn(
            DenseMatrix,
            Option<FeCoefficientBatch>,
            usize,
            FeCertificate,
        ) -> Result<CertifiedBorrowedMap>,
    ) -> Result<CertifiedBorrowedMap> {
        let Self {
            design,
            weights,
            config,
            compact,
        } = self;
        let mut iterations = 0_usize;
        while iterations < config.max_iterations {
            crate::interrupt::checkpoint()?;
            let x0 = (!compact)
                .then(|| fallible_matrix_copy(&values, "unable to reserve Irons-Tuck x0"))
                .transpose()?;
            let mut c0 = coefficients
                .as_ref()
                .map(FeCoefficientBatch::try_clone)
                .transpose()?;
            borrowed_residual_sweep_recording(
                design,
                weights,
                &mut values,
                &mut workspace,
                coefficients.as_mut(),
                config.symmetric,
            )?;
            iterations += 1;
            let mut certificate =
                certify(design, weights, &values, config.tolerance, &mut workspace)?;
            if certificate.passed {
                certificate = confirm(
                    &mut values,
                    &mut workspace,
                    coefficients.as_mut(),
                    &mut iterations,
                    certificate,
                )?;
                if !certificate.passed {
                    continue;
                }
                return finish(values, coefficients, iterations, certificate);
            }
            if iterations == config.max_iterations {
                break;
            }

            let x1 = (!compact)
                .then(|| fallible_matrix_copy(&values, "unable to reserve Irons-Tuck x1"))
                .transpose()?;
            let c1 = coefficients
                .as_ref()
                .map(FeCoefficientBatch::try_clone)
                .transpose()?;
            borrowed_residual_sweep_recording(
                design,
                weights,
                &mut values,
                &mut workspace,
                coefficients.as_mut(),
                config.symmetric,
            )?;
            iterations += 1;
            let mut baseline = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            if baseline.passed {
                baseline = confirm(
                    &mut values,
                    &mut workspace,
                    coefficients.as_mut(),
                    &mut iterations,
                    baseline,
                )?;
                if !baseline.passed {
                    continue;
                }
                return finish(values, coefficients, iterations, baseline);
            }

            let proposal = prepare_candidate(
                design,
                compact,
                x0.as_ref().zip(x1.as_ref()),
                &mut c0,
                c1.as_ref(),
                coefficients.as_ref(),
                &values,
            )?;
            if let Some(Candidate {
                values: candidate,
                coefficients: mut candidate_coefficients,
                observation_alpha,
            }) = proposal
            {
                let mut candidate_certificate = certify(
                    design,
                    weights,
                    &candidate,
                    config.tolerance,
                    &mut workspace,
                )?;
                if candidate_certificate.relative_two_norm < baseline.relative_two_norm {
                    if let Some(alpha) = observation_alpha {
                        candidate_coefficients = accepted_observation_coefficients(
                            c0.as_ref(),
                            c1.as_ref(),
                            coefficients.as_ref(),
                            alpha,
                        )?;
                    }
                    values = candidate;
                    coefficients = candidate_coefficients;
                    if candidate_certificate.passed {
                        candidate_certificate = confirm(
                            &mut values,
                            &mut workspace,
                            coefficients.as_mut(),
                            &mut iterations,
                            candidate_certificate,
                        )?;
                        if !candidate_certificate.passed {
                            continue;
                        }
                        return finish(values, coefficients, iterations, candidate_certificate);
                    }
                }
            }
        }

        Err(invalid_data(
            "borrowed Irons-Tuck exhausted its iteration budget without certification",
        ))
    }
}

// Retain the incumbent retained-IT allocation and cancellation order exactly.
// Compact IT consumes its first bank through a separate in-place helper.
fn accepted_observation_coefficients(
    first: Option<&FeCoefficientBatch>,
    second: Option<&FeCoefficientBatch>,
    third: Option<&FeCoefficientBatch>,
    alpha: f64,
) -> Result<Option<FeCoefficientBatch>> {
    let (Some(first), Some(second), Some(third)) = (first, second, third) else {
        return Ok(None);
    };
    if !first.same_layout(second) || !first.same_layout(third) {
        return Err(invalid_argument(
            "Irons-Tuck FE coefficient layouts are inconsistent",
        ));
    }
    if !alpha.is_finite() {
        return Err(invalid_data("Irons-Tuck FE coefficient scale is nonfinite"));
    }
    let mut candidate = first.try_clone()?;
    for index in 0..candidate.values.len() {
        let a = first.values[index];
        let b = second.values[index];
        let c = third.values[index];
        let d1 = b - a;
        let d2 = c - 2.0 * b + a;
        candidate.values[index] = alpha.mul_add(2.0 * d1, a).mul_add(1.0, alpha * alpha * d2);
        if !candidate.values[index].is_finite() {
            return Err(invalid_data(
                "Irons-Tuck FE coefficient extrapolation is nonfinite",
            ));
        }
    }
    Ok(Some(candidate))
}

struct Candidate {
    values: DenseMatrix,
    coefficients: Option<FeCoefficientBatch>,
    observation_alpha: Option<f64>,
}

fn prepare_candidate(
    design: &BorrowedInterceptFeDesign<'_>,
    compact: bool,
    snapshots: Option<(&DenseMatrix, &DenseMatrix)>,
    first: &mut Option<FeCoefficientBatch>,
    second: Option<&FeCoefficientBatch>,
    third: Option<&FeCoefficientBatch>,
    residual: &DenseMatrix,
) -> Result<Option<Candidate>> {
    if compact {
        Ok(compact_candidate(
            design,
            first.take().expect("compact initial coefficients"),
            second.expect("compact first coefficients"),
            third.expect("compact second coefficients"),
            residual,
        )?
        .map(|(values, coefficients)| Candidate {
            values,
            coefficients,
            observation_alpha: None,
        }))
    } else {
        let (first, second) = snapshots.expect("observation snapshots");
        Ok(
            observation_candidate(first, second, residual)?.map(|(values, alpha)| Candidate {
                values,
                coefficients: None,
                observation_alpha: Some(alpha),
            }),
        )
    }
}

fn observation_candidate(
    x0: &DenseMatrix,
    x1: &DenseMatrix,
    x2: &DenseMatrix,
) -> Result<Option<(DenseMatrix, f64)>> {
    if x0.nrows() != x1.nrows()
        || x0.ncols() != x1.ncols()
        || x0.nrows() != x2.nrows()
        || x0.ncols() != x2.ncols()
    {
        return Err(invalid_argument(
            "borrowed Irons-Tuck states have inconsistent dimensions",
        ));
    }
    let mut d1d1 = 0.0_f64;
    let mut d1d2 = 0.0_f64;
    let mut d2d2 = 0.0_f64;
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
    let alpha = (-d1d2 / d2d2).clamp(-8.0, 8.0);
    if !alpha.is_finite() {
        return Ok(None);
    }
    let mut candidate = fallible_matrix_copy(x0, "unable to reserve Irons-Tuck candidate")?;
    for (((output, a), b), c) in candidate
        .as_mut_slice()
        .iter_mut()
        .zip(x0.as_slice())
        .zip(x1.as_slice())
        .zip(x2.as_slice())
    {
        let d1 = b - a;
        let d2 = c - 2.0 * b + a;
        *output = a + 2.0 * alpha * d1 + alpha * alpha * d2;
        if !output.is_finite() {
            return Ok(None);
        }
    }
    Ok(Some((candidate, alpha)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{BorrowedInterceptEffect, LevelIndex};

    fn fixture<T>(
        repeats: usize,
        f: impl FnOnce(&BorrowedInterceptFeDesign<'_>, &[f64], DenseMatrix) -> T,
    ) -> T {
        // Two disconnected full rectangles with interleaved level identifiers.
        let n = repeats * 8;
        let a: Vec<u32> = (0..n).map(|i| [0, 0, 2, 2, 1, 1, 3, 3][i % 8]).collect();
        let b: Vec<u32> = (0..n).map(|i| [0, 2, 0, 2, 1, 3, 1, 3][i % 8]).collect();
        let ia = LevelIndex::from_codes(&a, 4).unwrap();
        let ib = LevelIndex::from_codes(&b, 4).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            n,
            vec![
                BorrowedInterceptEffect::new(&a, 4, &ia).unwrap(),
                BorrowedInterceptEffect::new(&b, 4, &ib).unwrap(),
            ],
        )
        .unwrap();
        let weights: Vec<_> = (0..n)
            .map(|i| [1., 2., 3., 5., 2., 6., 3., 7.][i % 8])
            .collect();
        let y: Vec<_> = (0..n)
            .map(|i| 1024. + [8., 2., -1., 4., -3., 6., 9., 1.][i % 8])
            .collect();
        let second = y.iter().map(|v| 3. * v + 7.).collect();
        f(
            &design,
            &weights,
            DenseMatrix::from_columns(&[y, second]).unwrap(),
        )
    }

    fn config() -> MapConfig {
        MapConfig {
            tolerance: 1e-12,
            max_iterations: 10_000,
            check_every: 1,
            symmetric: true,
        }
    }

    #[test]
    fn coefficient_it_matches_weighted_rectangle_witness_and_reconstructs() {
        fixture(16, |design, weights, original| {
            for retain in [false, true] {
                assert!(plan(design, 2, retain, None).unwrap().0);
                let result = absorb_borrowed_intercept_irons_tuck_certified(
                    design,
                    BorrowedMapWeights::Provided(weights),
                    original.clone(),
                    config(),
                    retain,
                    None,
                    true,
                )
                .unwrap();
                let (residual, report, coefficients) = result.into_full_parts();
                assert!(report.certified());
                assert_eq!(coefficients.is_some(), retain);
                for column in 0..2 {
                    for block in [0, 4] {
                        // Null(D'W) on a rectangle is span(sign_i / w_i).
                        let signs = [1., -1., -1., 1.];
                        let contrast: f64 = (0..4)
                            .map(|i| signs[i] * original[(block + i, column)])
                            .sum();
                        let denominator: f64 = (0..4).map(|i| 1. / weights[block + i]).sum();
                        for row in 0..design.nobs() {
                            if row % 8 / 4 != block / 4 {
                                continue;
                            }
                            let expected = contrast * signs[row % 4] / (denominator * weights[row]);
                            assert!((residual[(row, column)] - expected).abs() < 2e-7);
                        }
                    }
                    if let Some(ref coefficients) = coefficients {
                        let values = coefficients.rhs(column).unwrap();
                        for row in 0..design.nobs() {
                            let fitted: f64 = design
                                .effects()
                                .iter()
                                .enumerate()
                                .map(|(j, e)| {
                                    values[coefficients.effect_offsets[j] + e.codes()[row] as usize]
                                })
                                .sum();
                            assert!(
                                (original[(row, column)] - residual[(row, column)] - fitted).abs()
                                    < 2e-10
                            );
                        }
                    }
                }
            }
        });
    }

    #[test]
    fn large_coefficient_layout_keeps_observation_it_and_honors_exact_budget() {
        fixture(1, |design, weights, values| {
            let (compact, peak) = plan(design, 2, false, None).unwrap();
            assert!(!compact);
            let bytes = modeled_bytes(peak).unwrap();
            let budget = FeSolverMemoryBudget {
                execution_bytes: 17,
                usable_bytes: bytes + 17,
            };
            let result = absorb_borrowed_intercept_irons_tuck_certified(
                design,
                BorrowedMapWeights::Provided(weights),
                values.clone(),
                config(),
                false,
                Some(budget),
                true,
            )
            .unwrap();
            assert!(result.report.certified());
            assert_eq!(result.report.solver_workspace_peak, peak);
            let denied = FeSolverMemoryBudget {
                usable_bytes: budget.usable_bytes - 1,
                ..budget
            };
            let error = absorb_borrowed_intercept_irons_tuck_certified(
                design,
                BorrowedMapWeights::Provided(weights),
                values,
                config(),
                false,
                Some(denied),
                true,
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
        });
    }

    #[test]
    fn coefficient_it_does_not_overrun_one_sweep_budget() {
        fixture(16, |design, weights, values| {
            let config = MapConfig {
                max_iterations: 1,
                ..config()
            };
            let error = absorb_borrowed_intercept_irons_tuck_certified(
                design,
                BorrowedMapWeights::Provided(weights),
                values,
                config,
                true,
                None,
                true,
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidData);
        });
    }
}
