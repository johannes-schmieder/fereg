//! Bounded developer-only categorical warm starts on immutable original RHSs.
use super::*;
use crate::fe::routing::CategoricalPrefix;

#[derive(Default)]
pub(super) struct Attempt {
    pub report: three_fe_map::Attempt,
    pub seed: Option<FeCoefficientBatch>,
}

/// Prefix execution owns a private residual and always records original-slot
/// coefficients. Only the seed survives an uncertified prefix; LSMR recreates
/// its residual from the untouched original response and original design.
#[allow(clippy::too_many_arguments)]
pub(super) fn attempt(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    original: &DenseMatrix,
    map: MapConfig,
    arm: CategoricalPrefix,
    budget: Option<FeSolverMemoryBudget>,
    coefficient_bytes: u128,
    retained_bytes: u128,
    allowance: usize,
    retain: bool,
) -> Result<Attempt> {
    let limit = match arm {
        CategoricalPrefix::None => return Ok(Attempt::default()),
        CategoricalPrefix::Map4 => 4,
        CategoricalPrefix::CompactIt4 => 8,
    };
    if design.effects().len() < 3 || allowance <= limit {
        return Ok(Attempt::default());
    }
    let compact = arm == CategoricalPrefix::CompactIt4;
    let panel = original.as_slice().len() as u128 * 8;
    let workspace = BorrowedMapWorkspace::planned_payload_bytes(
        design,
        original.nrows(),
        original.ncols(),
        rayon::current_num_threads(),
    )?;
    let bytes = workspace
        .checked_add(retained_bytes)
        .and_then(|b| b.checked_add(panel.checked_mul(if compact { 2 } else { 1 })?))
        .and_then(|b| b.checked_add(coefficient_bytes.checked_mul(if compact { 3 } else { 1 })?))
        .and_then(|b| b.checked_add(1024));
    let Some(bytes) = bytes.filter(|&b| budget.is_none_or(|limit| limit.fits(b))) else {
        return Ok(Attempt::default());
    };
    let mut attempt = Attempt::default();
    attempt.report.peak = SolverWorkspacePeak::from_bytes(bytes)?;
    let setup = Instant::now();
    let result: Result<_> = (|| {
        let weights = validate_weights(weights, original.nrows())?;
        let values = fallible_matrix_copy(original, "unable to reserve categorical prefix")?;
        let coefficients = FeCoefficientBatch::intercept_zeros(design, original.ncols())?;
        let mut workspace = BorrowedMapWorkspace::new(
            design,
            original.nrows(),
            original.ncols(),
            &mut SystemWorkspaceReservation,
        )?;
        update_workspace_norms(original, weights, &mut workspace, true)?;
        Ok((weights, values, coefficients, workspace))
    })();
    attempt.report.setup_seconds = setup.elapsed().as_secs_f64();
    let (weights, values, coefficients, workspace) = match result {
        Ok(state) => state,
        Err(e) if matches!(e.code, ErrorCode::Allocation | ErrorCode::InvalidData) => {
            return Ok(attempt)
        }
        Err(e) => return Err(e),
    };
    let solve = Instant::now();
    let state = State {
        design,
        weights,
        map,
        values,
        coefficients,
        workspace,
        limit,
    };
    let result = state.run(compact, &mut attempt.report.iterations);
    attempt.report.solve_seconds = solve.elapsed().as_secs_f64();
    match result {
        Ok((state, certificate)) if certificate.passed => {
            let normal_equation_residual = certificate.relative_two_norm;
            let mut result = certified_solver_result(
                state.values,
                if compact {
                    FeSolverKind::IronsTuck
                } else {
                    FeSolverKind::Map
                },
                SolveStopReason::Certified,
                attempt.report.iterations,
                certificate,
                normal_equation_residual,
                attempt.report.setup_seconds,
                attempt.report.solve_seconds,
            )?;
            result.report.requested_solver = FeSolverKind::Auto;
            result.set_solver_workspace_peak(attempt.report.peak);
            result.report.pilot_iterations = attempt.report.iterations;
            attempt.report.result = Some(attach_optional_coefficients(
                result,
                retain.then_some(state.coefficients),
            )?);
        }
        Ok((state, _)) => attempt.seed = Some(state.coefficients),
        Err(e) if matches!(e.code, ErrorCode::Allocation | ErrorCode::InvalidData) => {}
        Err(e) => return Err(e),
    }
    Ok(attempt)
}

struct State<'a, 'd> {
    design: &'a BorrowedInterceptFeDesign<'d>,
    weights: ValidatedWeights<'a>,
    map: MapConfig,
    values: DenseMatrix,
    coefficients: FeCoefficientBatch,
    workspace: BorrowedMapWorkspace,
    limit: usize,
}

impl State<'_, '_> {
    fn certificate(&mut self) -> Result<FeCertificate> {
        certify(
            self.design,
            self.weights,
            &self.values,
            self.map.tolerance,
            &mut self.workspace,
        )
    }

    fn sweep(&mut self, iterations: &mut usize) -> Result<FeCertificate> {
        crate::interrupt::checkpoint()?;
        // Failed partial sweeps consume their action; no retry receives credit.
        *iterations += 1;
        borrowed_residual_sweep_recording(
            self.design,
            self.weights,
            &mut self.values,
            &mut self.workspace,
            Some(&mut self.coefficients),
            self.map.symmetric,
        )?;
        self.certificate()
    }

    fn confirm(
        &mut self,
        certificate: FeCertificate,
        iterations: &mut usize,
    ) -> Result<FeCertificate> {
        if certificate.passed && *iterations < self.limit {
            self.sweep(iterations)
        } else {
            Ok(certificate)
        }
    }

    fn run(mut self, compact: bool, iterations: &mut usize) -> Result<(Self, FeCertificate)> {
        let mut certificate = self.certificate()?;
        if certificate.passed {
            return Ok((self, certificate));
        }
        while *iterations < self.limit {
            let first = compact.then(|| self.coefficients.try_clone()).transpose()?;
            certificate = self.sweep(iterations)?;
            if certificate.passed {
                certificate = self.confirm(certificate, iterations)?;
                if certificate.passed {
                    break;
                }
                continue;
            }
            if !compact || *iterations == self.limit {
                continue;
            }
            let second = self.coefficients.try_clone()?;
            certificate = self.sweep(iterations)?;
            if certificate.passed {
                certificate = self.confirm(certificate, iterations)?;
                if certificate.passed {
                    break;
                }
                continue;
            }
            if let Some((candidate, coefficients)) = irons_tuck::compact_candidate(
                self.design,
                first.expect("compact prefix first state"),
                &second,
                &self.coefficients,
                &self.values,
            )? {
                let proposed = certify(
                    self.design,
                    self.weights,
                    &candidate,
                    self.map.tolerance,
                    &mut self.workspace,
                )?;
                if proposed.relative_two_norm < certificate.relative_two_norm {
                    self.values = candidate;
                    self.coefficients =
                        coefficients.expect("compact prefix candidate coefficients");
                    certificate = self.confirm(proposed, iterations)?;
                    if certificate.passed {
                        break;
                    }
                }
            }
        }
        Ok((self, certificate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

    fn fixture(run: impl FnOnce(&BorrowedInterceptFeDesign<'_>, &[f64], DenseMatrix)) {
        let n = 128;
        let codes: [Vec<u32>; 3] = std::array::from_fn(|j| {
            (0..n)
                .map(|r| match j {
                    0 => ((r / 8) % 2) as u32,
                    1 => (((r / 8) + usize::from(r == 0)) % 2) as u32,
                    _ => ((r / 2) % 2) as u32,
                })
                .collect()
        });
        let indices: Vec<_> = codes
            .iter()
            .map(|c| LevelIndex::from_codes(c, 2).unwrap())
            .collect();
        let sources: Vec<_> = codes
            .iter()
            .zip(&indices)
            .map(|(c, i)| ValidatedBorrowedInterceptSource::new(c, 2, i).unwrap())
            .collect();
        let design =
            BorrowedInterceptFeDesign::new(n, sources.iter().map(|s| s.effect()).collect())
                .unwrap();
        let weights: Vec<_> = (0..n).map(|r| 1. + (r % 5) as f64 / 4.).collect();
        let original = DenseMatrix::from_columns(&[(0..n)
            .map(|r| (r as f64 * 0.17).sin() + codes[0][r] as f64)
            .collect()])
        .unwrap();
        run(&design, &weights, original);
    }

    fn config() -> MapConfig {
        MapConfig {
            tolerance: 1e-14,
            max_iterations: 100,
            check_every: 1,
            symmetric: true,
        }
    }

    fn check_original_certificate(
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        original: &DenseMatrix,
        residual: &DenseMatrix,
        tolerance: f64,
    ) -> FeCertificate {
        let weights = validate_weights(weights, design.nobs()).unwrap();
        let mut workspace = BorrowedMapWorkspace::new(
            design,
            design.nobs(),
            original.ncols(),
            &mut SystemWorkspaceReservation,
        )
        .unwrap();
        update_workspace_norms(original, weights, &mut workspace, true).unwrap();
        certify(design, weights, residual, tolerance, &mut workspace).unwrap()
    }

    fn project<'a>(
        design: &BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'_>,
        original: &DenseMatrix,
        map: MapConfig,
        native_limit: usize,
        budget: Option<FeSolverMemoryBudget>,
        route: &mut routing_lsmr::GeneralRoutingState<'a>,
    ) -> CertifiedBorrowedMap {
        let levels: Vec<_> = design
            .effects()
            .iter()
            .map(|effect| effect.n_levels())
            .collect();
        let lsmr = LsmrConfig {
            max_iterations: native_limit,
            preconditioner: LsmrPreconditioner::Off,
            ..LsmrConfig::default()
        };
        let estimate = estimate_fe_solver_memory(
            design.nobs(),
            original.ncols(),
            &levels,
            rayon::current_num_threads(),
            lsmr.local_size,
        )
        .unwrap();
        project_general_categorical(
            design,
            weights,
            original.clone(),
            map,
            lsmr,
            budget,
            true,
            estimate,
            false,
            Some(4),
            Some(route),
            false,
        )
        .unwrap()
    }

    #[test]
    fn portfolio_v1_keeps_strict_certificate_at_prefix_and_final_exit() {
        fixture(|design, weights, original| {
            let weights = BorrowedMapWeights::Provided(weights);
            let coefficient_bytes = FeCoefficientBatch::intercept_zeros(design, 1)
                .unwrap()
                .retained_bytes();
            let unresolved = attempt(
                design,
                weights,
                &original,
                config(),
                CategoricalPrefix::Map4,
                None,
                coefficient_bytes,
                0,
                100,
                true,
            )
            .unwrap();
            let seed = unresolved.seed.unwrap();
            let mut residual = original.clone();
            for row in 0..design.nobs() {
                for (effect, term) in design.effects().iter().enumerate() {
                    residual[(row, 0)] -= seed.rhs(0).unwrap()
                        [seed.effect_offsets[effect] + term.codes()[row] as usize];
                }
            }
            let ratio = check_original_certificate(design, weights, &original, &residual, 1e-14)
                .relative_infinity_norm;
            let map = MapConfig {
                tolerance: 2.0 * ratio,
                ..config()
            };
            let strict = crate::fe::auto::direct_lsmr_tolerance(map.tolerance);
            assert!(ratio > strict && ratio < map.tolerance);
            let mut legacy = routing_lsmr::GeneralRoutingState::new(
                crate::fe::RoutingPreset::prefix(CategoricalPrefix::Map4),
            );
            let requested = project(design, weights, &original, map, 50, None, &mut legacy);
            assert_eq!(requested.report().used_solver, FeSolverKind::Map);
            assert!(!legacy.has_prepared());
            assert!(
                !check_original_certificate(
                    design,
                    weights,
                    &original,
                    requested.residualized(),
                    strict
                )
                .passed
            );
            let mut portfolio =
                routing_lsmr::GeneralRoutingState::new(crate::fe::RoutingPreset::portfolio_v1());
            let result = project(design, weights, &original, map, 50, None, &mut portfolio);
            assert_eq!(result.report().pilot_iterations, 4);
            assert!(result.report().iterations > 4 && result.report().iterations <= 50);
            assert!(
                check_original_certificate(
                    design,
                    weights,
                    &original,
                    result.residualized(),
                    strict
                )
                .passed
            );
            assert!(
                !portfolio.has_replacement(),
                "portfolio general stage never builds a pair replacement"
            );
        });
    }

    #[test]
    fn portfolio_v1_certifies_exact_prefix_and_declines_without_stacking_pilots() {
        fixture(|design, _, original| {
            let preset = crate::fe::RoutingPreset::portfolio_v1();
            let map = MapConfig {
                tolerance: 1e-8,
                max_iterations: 256,
                ..config()
            };
            let exact = DenseMatrix::from_columns(&[design.effects()[0]
                .codes()
                .iter()
                .map(|&code| f64::from(code))
                .collect()])
            .unwrap();
            let mut route = routing_lsmr::GeneralRoutingState::new(preset);
            let result = project(
                design,
                BorrowedMapWeights::Unit,
                &exact,
                map,
                128,
                None,
                &mut route,
            );
            assert_eq!(result.report().used_solver, FeSolverKind::Map);
            assert_eq!(
                result.report().pilot_iterations,
                2,
                "passing first sweep includes confirmation"
            );
            assert!(!route.has_prepared());
            assert!(result
                .residualized()
                .as_slice()
                .iter()
                .all(|&value| value == 0.0));
            let mut capped = routing_lsmr::GeneralRoutingState::new(preset);
            let result = project(
                design,
                BorrowedMapWeights::Unit,
                &original,
                map,
                4,
                None,
                &mut capped,
            );
            assert_eq!(
                result.report().pilot_iterations,
                0,
                "four-step native cap cannot fund prefix plus LSMR"
            );
            let estimate = estimate_fe_solver_memory(
                design.nobs(),
                1,
                &[2, 2, 2],
                rayon::current_num_threads(),
                LsmrConfig::default().local_size,
            )
            .unwrap();
            let budget = FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: estimate.lsmr_bytes(LsmrPreconditioner::Off),
            };
            let mut constrained = routing_lsmr::GeneralRoutingState::new(preset);
            let result = project(
                design,
                BorrowedMapWeights::Unit,
                &original,
                map,
                128,
                Some(budget),
                &mut constrained,
            );
            assert_eq!(
                result.report().pilot_iterations,
                0,
                "seed admission refusal cannot enable the old 3FE pilot"
            );
            assert!(
                u128::from(result.report().solver_workspace_peak.bytes().unwrap())
                    <= budget.usable_bytes
            );
            assert!(result.report().certified());
            assert!(!constrained.has_replacement());
        });
    }

    #[test]
    fn bounded_prefix_seeds_reconstruct_weighted_original_slots() {
        fixture(|design, weights, original| {
            let saved = original.as_slice().to_vec();
            let coefficient_bytes = FeCoefficientBatch::intercept_zeros(design, 1)
                .unwrap()
                .retained_bytes();
            for (arm, cap) in [
                (CategoricalPrefix::Map4, 4),
                (CategoricalPrefix::CompactIt4, 8),
            ] {
                let trial = attempt(
                    design,
                    BorrowedMapWeights::Provided(weights),
                    &original,
                    config(),
                    arm,
                    None,
                    coefficient_bytes,
                    0,
                    100,
                    false,
                )
                .unwrap();
                assert!(trial.report.result.is_none());
                assert_eq!(trial.report.iterations, cap);
                let seed = trial.seed.unwrap();
                let coefficients = seed.rhs(0).unwrap();
                let mut residual = original.clone();
                for row in 0..design.nobs() {
                    for (j, effect) in design.effects().iter().enumerate() {
                        residual[(row, 0)] -=
                            coefficients[seed.effect_offsets[j] + effect.codes()[row] as usize];
                    }
                }
                assert!(residual.as_slice().iter().all(|v| v.is_finite()));
                // Independently replay MAP for its known linear seed, without IT.
                if arm == CategoricalPrefix::Map4 {
                    let validated =
                        validate_weights(BorrowedMapWeights::Provided(weights), design.nobs())
                            .unwrap();
                    let mut workspace = BorrowedMapWorkspace::new(
                        design,
                        design.nobs(),
                        1,
                        &mut SystemWorkspaceReservation,
                    )
                    .unwrap();
                    let mut replay = original.clone();
                    for _ in 0..4 {
                        borrowed_residual_sweep_recording::<FeCoefficientBatch>(
                            design,
                            validated,
                            &mut replay,
                            &mut workspace,
                            None,
                            true,
                        )
                        .unwrap();
                    }
                    for (actual, expected) in residual.as_slice().iter().zip(replay.as_slice()) {
                        assert!((actual - expected).abs() < 2e-12);
                    }
                }
                assert_eq!(original.as_slice(), saved);
                let bytes = modeled_bytes(trial.report.peak).unwrap();
                let refused = attempt(
                    design,
                    BorrowedMapWeights::Provided(weights),
                    &original,
                    config(),
                    arm,
                    Some(FeSolverMemoryBudget {
                        execution_bytes: 0,
                        usable_bytes: bytes - 1,
                    }),
                    coefficient_bytes,
                    0,
                    100,
                    false,
                )
                .unwrap();
                assert!(refused.seed.is_none());
                assert_eq!(refused.report.iterations, 0);
                let exhausted = attempt(
                    design,
                    BorrowedMapWeights::Provided(weights),
                    &original,
                    config(),
                    arm,
                    None,
                    coefficient_bytes,
                    0,
                    cap,
                    false,
                )
                .unwrap();
                assert!(exhausted.seed.is_none());
                assert_eq!(exhausted.report.iterations, 0);
            }
        });
    }

    #[test]
    fn zero_prefix_is_exact_and_degenerate_it_proposal_is_rejected() {
        fixture(|design, _, original| {
            let trial = attempt(
                design,
                BorrowedMapWeights::Unit,
                &original,
                config(),
                CategoricalPrefix::None,
                None,
                0,
                0,
                100,
                false,
            )
            .unwrap();
            assert!(trial.seed.is_none());
            assert_eq!(trial.report.iterations, 0);
            let coefficients = FeCoefficientBatch::intercept_zeros(design, 1).unwrap();
            let proposal = irons_tuck::compact_candidate(
                design,
                coefficients.try_clone().unwrap(),
                &coefficients,
                &coefficients,
                &original,
            )
            .unwrap();
            assert!(proposal.is_none());
        });
    }

    #[test]
    fn each_changed_rhs_repeats_prefix_with_a_retained_backend() {
        let rows = 544;
        let levels = [16, 16, 6];
        let codes: [Vec<u32>; 3] = std::array::from_fn(|effect| {
            (0..rows)
                .map(|row| {
                    let component = row / 272;
                    let cell = (row % 272) / 2;
                    let worker = cell / 17;
                    match effect {
                        0 => (8 * component + worker) as u32,
                        1 => (8 * component + (worker + usize::from(cell % 17 == 16)) % 8) as u32,
                        _ => (3 * component + (cell * 7 + cell / 11) % 3) as u32,
                    }
                })
                .collect()
        });
        let indices: Vec<_> = codes
            .iter()
            .zip(levels)
            .map(|(c, levels)| LevelIndex::from_codes(c, levels).unwrap())
            .collect();
        let sources: Vec<_> = codes
            .iter()
            .zip(&indices)
            .zip(levels)
            .map(|((c, index), levels)| {
                ValidatedBorrowedInterceptSource::new(c, levels, index).unwrap()
            })
            .collect();
        let design =
            BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect())
                .unwrap();
        let weights: Vec<_> = (0..rows)
            .map(|row| 1. + (row / 2 % 7) as f64 / 4.)
            .collect();
        let map = MapConfig {
            tolerance: 1e-10,
            max_iterations: 256,
            ..config()
        };
        let lsmr = LsmrConfig {
            max_iterations: 128,
            preconditioner: LsmrPreconditioner::Off,
            ..LsmrConfig::default()
        };
        for (preset, sweeps) in [
            (crate::fe::RoutingPreset::prefix(CategoricalPrefix::Map4), 4),
            (
                crate::fe::RoutingPreset::prefix(CategoricalPrefix::CompactIt4),
                8,
            ),
            (crate::fe::RoutingPreset::portfolio_v1(), 4),
        ] {
            let mut route = routing_lsmr::GeneralRoutingState::new(preset);
            for action in 0..2 {
                assert_eq!(route.has_prepared(), action > 0);
                // Opposite equal-weight rows share every FE membership, so the
                // exact within residual is their signed noise, independently
                // of either prefix or the retained iterative backend.
                let noise: Vec<_> = (0..rows)
                    .map(|row| {
                        if row % 2 == 0 {
                            -0.25 * (action + 1) as f64
                        } else {
                            0.25 * (action + 1) as f64
                        }
                    })
                    .collect();
                let column: Vec<_> = (0..rows)
                    .map(|row| {
                        noise[row]
                            + codes
                                .iter()
                                .enumerate()
                                .map(|(j, c)| {
                                    (((j + 1 + action) as f64) * c[row] as f64
                                        + 0.3 * action as f64)
                                        .cos()
                                })
                                .sum::<f64>()
                    })
                    .collect();
                let columns: Vec<Vec<_>> = (1..=action + 1)
                    .map(|scale| column.iter().map(|x| scale as f64 * x).collect())
                    .collect();
                let input = DenseMatrix::from_columns(&columns).unwrap();
                let result = project_borrowed_intercept_structural(
                    &design,
                    BorrowedMapWeights::Provided(&weights),
                    input.clone().into(),
                    map,
                    lsmr,
                    None,
                    false,
                    true,
                    false,
                    false,
                    None,
                    Some(&mut route),
                )
                .unwrap();
                assert!(route.has_prepared());
                let report = result.report();
                assert_eq!(report.used_solver, FeSolverKind::Lsmr, "{report:?}");
                assert_eq!(report.pilot_iterations, sweeps);
                assert!(report.iterations > sweeps && report.iterations <= lsmr.max_iterations);
                assert!(!report.fallback_used);
                for column in 0..input.ncols() {
                    for (row, expected) in noise.iter().enumerate() {
                        assert!(
                            (result.residualized()[(row, column)] - (column + 1) as f64 * expected)
                                .abs()
                                < 1e-8
                        );
                    }
                }
                let validated =
                    validate_weights(BorrowedMapWeights::Provided(&weights), rows).unwrap();
                let mut workspace = BorrowedMapWorkspace::new(
                    &design,
                    rows,
                    input.ncols(),
                    &mut SystemWorkspaceReservation,
                )
                .unwrap();
                update_workspace_norms(&input, validated, &mut workspace, true).unwrap();
                assert!(
                    certify(
                        &design,
                        validated,
                        result.residualized(),
                        if preset.strict_categorical_prefix() {
                            crate::fe::auto::direct_lsmr_tolerance(map.tolerance)
                        } else {
                            map.tolerance
                        },
                        &mut workspace
                    )
                    .unwrap()
                    .passed
                );
            }
        }
    }
}
