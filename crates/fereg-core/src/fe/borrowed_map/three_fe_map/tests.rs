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
        let result = attempt(design, &values, map, None, false, 0, 1).unwrap();
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
        )
        .unwrap();
        assert!(refused.result.is_none());
        assert_eq!(refused.iterations, 0);
        assert_eq!(refused.peak, SolverWorkspacePeak::ZERO);
        let failed = attempt(design, &values, map, None, false, 0, 1).unwrap();
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
            estimate_fe_solver_memory(values.nrows(), values.ncols(), &[2; 3], 1, None).unwrap();
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
        let refused = attempt(design, &values, MapConfig::default(), None, false, 0, 1).unwrap();
        assert!(refused.result.is_none());
        assert_eq!(refused.iterations, 0);
        assert!(refused.peak.bytes().unwrap() >= values.as_slice().len() as u64 * 8);
        SETUP_FAILURE.set(Some(ErrorCode::Internal));
        assert!(
            matches!(attempt(design, &values, MapConfig::default(), None, false, 0, 1),
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
        )
        .unwrap();
        assert!(overflow.result.is_none());
        assert_eq!(overflow.peak, SolverWorkspacePeak::ZERO);
    });
}

#[test]
fn pilot_debits_first_half_work_and_keeps_the_final_reserve() {
    fixture(true, |design, values| {
        let estimate =
            estimate_fe_solver_memory(values.nrows(), values.ncols(), &[2; 3], 1, None).unwrap();
        let map = MapConfig {
            tolerance: 1e-10,
            max_iterations: 80,
            ..MapConfig::default()
        };
        let result = super::super::project_general_categorical(
            design,
            BorrowedMapWeights::Unit,
            values.clone(),
            map,
            LsmrConfig::default(),
            None,
            false,
            estimate,
            false,
            Some(1),
            None,
            false,
        )
        .unwrap();
        assert_eq!(result.report.pilot_iterations, SWEEPS);
        assert!(result.report.fallback_used);
        assert_eq!(result.report.used_solver, FeSolverKind::Lsmr);
        assert!(result.report.iterations <= map.max_iterations / 2);
        let short = MapConfig {
            max_iterations: 8,
            ..map
        };
        let without = super::super::project_general_categorical(
            design,
            BorrowedMapWeights::Unit,
            values.clone(),
            short,
            LsmrConfig::default(),
            None,
            false,
            estimate,
            false,
            None,
            None,
            false,
        )
        .unwrap();
        let skipped = super::super::project_general_categorical(
            design,
            BorrowedMapWeights::Unit,
            values,
            short,
            LsmrConfig::default(),
            None,
            false,
            estimate,
            false,
            Some(1),
            None,
            false,
        )
        .unwrap();
        assert_eq!(skipped.report.pilot_iterations, 0);
        assert_eq!(skipped.report.iterations, without.report.iterations);
        assert_eq!(skipped.report.used_solver, without.report.used_solver);
        assert_eq!(skipped.residualized(), without.residualized());
    });
}

#[test]
fn consumed_pilot_keeps_fresh_lsmr_norms_and_retained_coefficients() {
    fixture(true, |design, values| {
        let estimate =
            estimate_fe_solver_memory(values.nrows(), values.ncols(), &[2; 3], 1, None).unwrap();
        let map = MapConfig {
            tolerance: 1e-10,
            max_iterations: 80,
            ..MapConfig::default()
        };
        let solve = |pilot: bool, budget| {
            super::super::project_general_categorical(
                design,
                BorrowedMapWeights::Unit,
                values.clone(),
                map,
                LsmrConfig::default(),
                budget,
                true,
                estimate,
                true,
                pilot.then_some(1),
                None,
                false,
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
        assert_eq!(candidate.report.used_solver, FeSolverKind::Lsmr);
        assert_eq!(candidate.residualized(), reference.residualized());
        assert_eq!(
            candidate.coefficients.as_ref().unwrap().values,
            reference.coefficients.as_ref().unwrap().values
        );
        assert_eq!(candidate.residual_norms, reference.residual_norms);
        assert_eq!(candidate.residual_norms.len(), values.ncols());
        assert!(candidate.report.solver_workspace_peak.bytes().unwrap() <= bytes);
        for (col, norm) in candidate.residual_norms.iter().enumerate() {
            let expected = crate::fe::norms::reference::legacy_scalar_norm_state(
                (0..values.nrows()).map(|r| candidate.residualized()[(r, col)]),
            )
            .0;
            assert_eq!(norm.to_bits(), expected.to_bits());
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
            matches!(attempt(design, &values, MapConfig::default(), None, false, 0, 1), Err(e) if e.code == ErrorCode::Interrupted)
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
            estimate_fe_solver_memory(values.nrows(), values.ncols(), &[2; 3], 1, None).unwrap();
        let earlier = attempt(
            design,
            &values,
            map,
            None,
            true,
            estimate.coefficient_batch_bytes,
            3,
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
