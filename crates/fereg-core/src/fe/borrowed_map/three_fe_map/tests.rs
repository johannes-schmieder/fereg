use super::*;
use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

fn fixture(weak: bool, run: impl FnOnce(&BorrowedInterceptFeDesign<'_>, DenseMatrix)) {
    let rows = 256;
    let codes: [Vec<u32>; 3] = std::array::from_fn(|effect| {
        (0..rows)
            .map(|r| {
                if weak && effect == 1 {
                    ((r / 8 + usize::from(r == 0)) % 2) as u32
                } else {
                    ((r / [8, 4, 2][effect]) % 2) as u32
                }
            })
            .collect()
    });
    let indexes: Vec<_> = codes
        .iter()
        .map(|c| LevelIndex::from_codes(c, 2).unwrap())
        .collect();
    let sources: Vec<_> = codes
        .iter()
        .zip(&indexes)
        .map(|(c, i)| ValidatedBorrowedInterceptSource::new(c, 2, i).unwrap())
        .collect();
    let design =
        BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect()).unwrap();
    let values = DenseMatrix::from_row_major(
        rows,
        2,
        (0..rows)
            .flat_map(|r| {
                [
                    (r as f64 * 0.17).sin() + codes[0][r] as f64,
                    (r as f64 * 0.31).cos() + codes[1][r] as f64,
                ]
            })
            .collect(),
    )
    .unwrap();
    run(&design, values);
}

#[test]
fn geometry_is_optional_and_keeps_additional_common_directions() {
    for weak in [false, true] {
        fixture(weak, |design, _| {
            let (geometry, peak) = Geometry::inspect(design, None).unwrap();
            assert_eq!(peak.bytes(), Some(16));
            assert_eq!(
                geometry.unwrap().predicted_sweeps(1e-10),
                if weak { None } else { Some(1) }
            );
            let (refused, peak) = Geometry::inspect(
                design,
                Some(FeSolverMemoryBudget {
                    execution_bytes: 10,
                    usable_bytes: 25,
                }),
            )
            .unwrap();
            assert!(refused.is_none());
            assert_eq!(peak, SolverWorkspacePeak::ZERO);
        });
    }
}

#[test]
fn passing_requested_certificate_is_accepted_without_a_tighter_gate() {
    fixture(false, |design, values| {
        let map = MapConfig {
            tolerance: 0.9,
            ..MapConfig::default()
        };
        let result = attempt(design, &values, map, None, false, 0, 1, false).unwrap();
        assert_eq!(result.iterations, 0);
        let result = result.result.unwrap();
        assert!(result.report.certified());
        assert!(
            result.report.certificate.relative_infinity_norm
                > super::super::super::auto::direct_lsmr_tolerance(map.tolerance)
        );
        assert_eq!(result.residualized(), &values);
        assert!(result.residual_norms.is_empty());
    });
}

#[test]
fn refused_or_failed_attempt_preserves_original_and_records_work() {
    fixture(true, |design, values| {
        let original = values.clone();
        let map = MapConfig {
            tolerance: 1e-14,
            ..MapConfig::default()
        };
        let refused = attempt(
            design,
            &values,
            map,
            Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: 0,
            }),
            false,
            0,
            1,
            false,
        )
        .unwrap();
        assert!(refused.result.is_none());
        assert_eq!(refused.iterations, 0);
        assert_eq!(refused.peak, SolverWorkspacePeak::ZERO);
        let failed = attempt(design, &values, map, None, false, 0, 1, false).unwrap();
        assert!(failed.result.is_none());
        assert_eq!(failed.iterations, SWEEPS);
        assert_eq!(values, original);
        assert!(failed.peak.bytes().unwrap() > values.as_slice().len() as u64 * 8);
    });
}

#[test]
fn successful_candidate_preserves_coefficients_and_admission() {
    fixture(false, |design, values| {
        let estimate =
            estimate_fe_solver_memory(values.nrows(), values.ncols(), &[2; 3], 1).unwrap();
        let map = MapConfig {
            tolerance: 1e-12,
            ..MapConfig::default()
        };
        let trial = attempt(
            design,
            &values,
            map,
            None,
            true,
            estimate.coefficient_batch_bytes,
            1,
            false,
        )
        .unwrap();
        let bytes = trial.peak.bytes().unwrap();
        let result = trial.result.unwrap();
        assert!(result.report.certified());
        assert_eq!(trial.iterations, 1);
        let coefficients = result.coefficients.as_ref().unwrap();
        for row in 0..values.nrows() {
            for rhs in 0..values.ncols() {
                let fitted: f64 = design
                    .effects()
                    .iter()
                    .enumerate()
                    .map(|(e, effect)| {
                        coefficients.values[rhs * 6 + e * 2 + effect.codes()[row] as usize]
                    })
                    .sum();
                assert!(
                    (fitted + result.residualized()[(row, rhs)] - values[(row, rhs)]).abs() < 1e-14
                );
            }
        }
        assert!(result.report.solver_workspace_peak.bytes().unwrap() <= bytes);
        let admitted = attempt(
            design,
            &values,
            map,
            Some(FeSolverMemoryBudget {
                execution_bytes: 123,
                usable_bytes: bytes as u128 + 123,
            }),
            true,
            estimate.coefficient_batch_bytes,
            1,
            false,
        )
        .unwrap();
        assert_eq!(
            admitted.result.unwrap().residualized(),
            result.residualized()
        );
    });
}

#[test]
fn partial_optional_allocation_refusal_drops_candidate_but_fatal_errors_propagate() {
    fixture(false, |design, values| {
        SETUP_FAILURE.set(Some(ErrorCode::Allocation));
        let refused = attempt(
            design,
            &values,
            MapConfig::default(),
            None,
            false,
            0,
            1,
            false,
        )
        .unwrap();
        assert!(refused.result.is_none());
        assert_eq!(refused.iterations, 0);
        assert!(refused.peak.bytes().unwrap() >= values.as_slice().len() as u64 * 8);
        SETUP_FAILURE.set(Some(ErrorCode::Internal));
        assert!(
            matches!(attempt(design, &values, MapConfig::default(), None, false, 0, 1, false),
            Err(e) if e.code == ErrorCode::Internal)
        );
        let overflow = attempt(
            design,
            &values,
            MapConfig::default(),
            None,
            true,
            u128::MAX,
            1,
            false,
        )
        .unwrap();
        assert!(overflow.result.is_none());
        assert_eq!(overflow.peak, SolverWorkspacePeak::ZERO);
    });
}

#[test]
fn pilot_debits_its_work_from_the_map_budget_and_keeps_the_final_reserve() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    fixture(true, |design, values| {
        let estimate =
            estimate_fe_solver_memory(values.nrows(), values.ncols(), &[2; 3], 1).unwrap();
        let map = MapConfig {
            tolerance: 1e-10,
            max_iterations: 16_000,
            ..MapConfig::default()
        };
        let solve = |map, pilot: Option<usize>| {
            super::super::project_general_categorical(
                design,
                BorrowedMapWeights::Unit,
                values.clone(),
                map,
                None,
                false,
                estimate,
                false,
                pilot,
                None,
            )
        };
        // A missed pilot's sweeps are debited; MAP then restarts from the
        // original inputs, so its result is the pilot-free one.
        let without = solve(map, None).unwrap();
        let result = solve(map, Some(1)).unwrap();
        assert_eq!(result.report.pilot_iterations, SWEEPS);
        assert!(result.report.fallback_used);
        assert_eq!(result.report.used_solver, FeSolverKind::Map);
        assert_eq!(result.report.iterations, without.report.iterations + SWEEPS);
        assert_eq!(result.residualized(), without.residualized());
        // The pilot needs more than its sweeps in half the budget.
        let short = MapConfig {
            max_iterations: 8,
            ..map
        };
        let without = solve(short, None);
        let skipped = solve(short, Some(1));
        match (without, skipped) {
            (Ok(without), Ok(skipped)) => {
                assert_eq!(skipped.report.pilot_iterations, 0);
                assert_eq!(skipped.report.iterations, without.report.iterations);
                assert_eq!(skipped.residualized(), without.residualized());
            }
            (Err(without), Err(skipped)) => assert_eq!(without, skipped),
            _ => panic!("the skipped pilot changed the outcome"),
        }
    });
}

#[test]
fn consumed_pilot_keeps_the_map_fallback_norms_and_retained_coefficients() {
    fixture(true, |design, values| {
        let estimate =
            estimate_fe_solver_memory(values.nrows(), values.ncols(), &[2; 3], 1).unwrap();
        let map = MapConfig {
            tolerance: 1e-10,
            max_iterations: 16_000,
            ..MapConfig::default()
        };
        let solve = |pilot: bool, budget| {
            super::super::project_general_categorical(
                design,
                BorrowedMapWeights::Unit,
                values.clone(),
                map,
                budget,
                true,
                estimate,
                true,
                pilot.then_some(1),
                None,
            )
            .unwrap()
        };
        let reference = solve(false, None);
        let first = solve(true, None);
        let bytes = first.report.solver_workspace_peak.bytes().unwrap();
        let candidate = solve(
            true,
            Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: bytes as u128,
            }),
        );
        assert_eq!(candidate.report.pilot_iterations, SWEEPS);
        assert!(candidate.report.fallback_used);
        assert_eq!(candidate.report.used_solver, FeSolverKind::Map);
        assert_eq!(candidate.residualized(), reference.residualized());
        assert_eq!(
            candidate.coefficients.as_ref().unwrap().values,
            reference.coefficients.as_ref().unwrap().values
        );
        assert_eq!(candidate.residual_norms, reference.residual_norms);
        assert!(candidate.report.solver_workspace_peak.bytes().unwrap() <= bytes);
        // The coefficients reproduce the projection.
        let coefficients = candidate.coefficients.as_ref().unwrap();
        for col in 0..values.ncols() {
            let rhs = coefficients.rhs(col).unwrap();
            for row in 0..values.nrows() {
                let fitted: f64 = design
                    .effects()
                    .iter()
                    .enumerate()
                    .map(|(e, effect)| {
                        rhs[coefficients.effect_offsets()[e] + effect.codes()[row] as usize]
                    })
                    .sum();
                let projected = values[(row, col)] - candidate.residualized()[(row, col)];
                assert!((fitted - projected).abs() <= 1e-8, "{fitted} {projected}");
            }
        }
    });
}

#[test]
fn geometry_and_pilot_propagate_break() {
    const CHILD: &str = "FEREG_H2_BREAK_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([
                "fe::borrowed_map::three_fe_map::tests::geometry_and_pilot_propagate_break",
                "--exact",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    fixture(false, |design, values| {
        crate::interrupt::request_cancellation();
        assert_eq!(
            Geometry::inspect(design, None).unwrap_err().code,
            ErrorCode::Interrupted
        );
        assert!(
            matches!(attempt(design, &values, MapConfig::default(), None, false, 0, 1, false), Err(e) if e.code == ErrorCode::Interrupted)
        );
        crate::interrupt::reset_cancellation();
        assert!(Geometry::inspect(design, None).unwrap().0.is_some());
    });
}

#[test]
fn missed_prediction_still_accepts_the_fourth_sweep_without_changing_its_result() {
    fixture(true, |design, values| {
        // This correlated design has relative infinity residuals above 0.0104
        // after sweep three and below it after sweep four.
        let map = MapConfig {
            tolerance: 0.0104,
            ..MapConfig::default()
        };
        let estimate =
            estimate_fe_solver_memory(values.nrows(), values.ncols(), &[2; 3], 1).unwrap();
        let earlier = attempt(
            design,
            &values,
            map,
            None,
            true,
            estimate.coefficient_batch_bytes,
            3,
            false,
        )
        .unwrap();
        let terminal = attempt(
            design,
            &values,
            map,
            None,
            true,
            estimate.coefficient_batch_bytes,
            SWEEPS,
            false,
        )
        .unwrap();
        assert_eq!(earlier.iterations, SWEEPS);
        assert_eq!(terminal.iterations, SWEEPS);
        assert_eq!(earlier.peak, terminal.peak);
        let earlier = earlier.result.unwrap();
        let terminal = terminal.result.unwrap();
        assert!(earlier.report.certified());
        assert_eq!(earlier.residualized(), terminal.residualized());
        assert_eq!(
            earlier.coefficients.unwrap().values,
            terminal.coefficients.unwrap().values
        );
    });
}

#[test]
fn early_certificate_accepts_one_sweep_before_the_prediction_with_identical_bits() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    fixture(false, |design, values| {
        let map = MapConfig {
            tolerance: 1e-12,
            ..MapConfig::default()
        };
        // The fixture certifies after one sweep. A prediction of two sweeps now
        // also certifies after the first, matching a direct one-sweep request.
        let direct = attempt(design, &values, map, None, true, 4096, 1, false).unwrap();
        let early = attempt(design, &values, map, None, true, 4096, 2, false).unwrap();
        assert_eq!(direct.iterations, 1);
        assert_eq!(early.iterations, 1);
        let (direct, early) = (direct.result.unwrap(), early.result.unwrap());
        assert!(early.report.certified());
        assert_eq!(direct.residualized(), early.residualized());
        assert_eq!(
            direct.coefficients.unwrap().values,
            early.coefficients.unwrap().values
        );
    });
}

#[test]
fn dense_norm_handoff_matches_the_lsmr_kernel_and_an_independent_sum() {
    fixture(false, |design, values| {
        let map = MapConfig {
            tolerance: 1e-12,
            ..MapConfig::default()
        };
        let plain = attempt(design, &values, map, None, false, 0, 1, false).unwrap();
        let handed = attempt(design, &values, map, None, false, 0, 1, true).unwrap();
        let (plain, handed) = (plain.result.unwrap(), handed.result.unwrap());
        assert!(plain.residual_norms.is_empty());
        assert_eq!(plain.residualized(), handed.residualized());
        let accepted = handed.residualized();
        let columns = accepted.ncols();
        let mut expected = vec![0.0; columns];
        let mut scales = vec![0.0; columns];
        let mut sums = vec![0.0; columns];
        let mut dense = vec![0.0; columns];
        let valid = crate::fe::norms::column_norms_with_dense(
            accepted,
            |_| Ok(1.0),
            &mut expected,
            &mut scales,
            &mut sums,
            &mut dense,
            "value",
            "norm",
        )
        .unwrap();
        assert!(valid);
        assert_eq!(handed.residual_norms, dense);
        for (column, &norm) in handed.residual_norms.iter().enumerate() {
            let direct: f64 = (0..accepted.nrows())
                .map(|row| accepted[(row, column)].powi(2))
                .sum::<f64>()
                .sqrt();
            assert!((norm - direct).abs() <= 1e-14 * direct.max(1.0));
        }
        // An initial-certificate exit keeps the caller's own norm kernel.
        let initial = attempt(
            design,
            &values,
            MapConfig {
                tolerance: 0.9,
                ..map
            },
            None,
            false,
            0,
            1,
            true,
        )
        .unwrap();
        assert_eq!(initial.iterations, 0);
        assert!(initial.result.unwrap().residual_norms.is_empty());
    });
}
