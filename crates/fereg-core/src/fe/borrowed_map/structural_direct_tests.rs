use super::*;

use super::super::structural_constant_tests::reference;

#[test]
fn direct_structural_projection_retains_raw_and_within_bits() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    for rows in [17, 513, 16_385] {
        for columns in [1, 3, 64, 65, 130] {
            let matrix = DenseMatrix::from_row_major(
                rows,
                columns,
                (0..rows)
                    .flat_map(|r| {
                        (0..columns).map(move |c| {
                            let v = (r % 19) as f64 / 8. - 1.;
                            match c % 7 {
                                0 => 1.,
                                1 => 1e12 + v,
                                2 => 1e150 * v,
                                3 => 1e-150 * v,
                                4 => 0.,
                                5 => -0.,
                                _ => v,
                            }
                        })
                    })
                    .collect(),
            )
            .unwrap();
            let (_, maxima, means, corrections) =
                reference::scalar_means(matrix.as_slice(), columns, None);
            let design = BorrowedInterceptFeDesign::new(rows, Vec::new()).unwrap();
            for workers in [1, 4, 16] {
                crate::ThreadTeam::new(
                    crate::ThreadConfig::from_requested(workers, false).unwrap(),
                )
                .install(|| {
                    let weights = validate_weights(BorrowedMapWeights::Unit, rows).unwrap();
                    let mut original = BorrowedMapWorkspace::new(
                        &design,
                        rows,
                        columns,
                        &mut SystemWorkspaceReservation,
                    )
                    .unwrap();
                    update_workspace_norms(&matrix, weights, &mut original, true).unwrap();
                    let mut fused = BorrowedMapWorkspace::new(
                        &design,
                        rows,
                        columns,
                        &mut SystemWorkspaceReservation,
                    )
                    .unwrap();
                    raw_statistics(&matrix, &maxima, &mut fused).unwrap();
                    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
                    assert_eq!(
                        bits(&original.reference_norms),
                        bits(&fused.reference_norms)
                    );
                    assert_eq!(bits(&means), bits(&fused.group_sums));
                    assert_eq!(bits(&corrections), bits(&fused.group_corrections));
                    // The exact ratio of the generated constant in the original
                    // complete certificate owns the rejection-screen boundary.
                    let root = (rows as f64).sqrt();
                    let boundary = (1. / root) * root;
                    for tolerance in [1e-12, f64::from_bits(boundary.to_bits() - 1), boundary, 2.] {
                        let config = AbsorbConfig {
                            map: MapConfig {
                                tolerance,
                                ..MapConfig::default()
                            },
                            ..AbsorbConfig::default()
                        };
                        let mut expected = project_direct_categorical(
                            &design,
                            BorrowedMapWeights::Unit,
                            matrix.clone(),
                            config.map,
                            None,
                            true,
                        )
                        .unwrap();
                        let mut actual = project(
                            &design,
                            matrix.clone().into(),
                            RawMaxima {
                                scales: &maxima,
                                constant: 0,
                            },
                            config,
                        )
                        .unwrap();
                        assert_eq!(
                            bits(expected.residualized().as_slice()),
                            bits(actual.residualized().as_slice())
                        );
                        assert_eq!(
                            bits(&expected.take_residual_norms()),
                            bits(&actual.take_residual_norms())
                        );
                        let strip = |report: &SolveReport| {
                            let mut r = report.clone();
                            r.setup_seconds = 0.;
                            r.solve_seconds = 0.;
                            r.certification_seconds = 0.;
                            r
                        };
                        assert_eq!(
                            strip(expected.report()),
                            strip(actual.report()),
                            "rows={rows} columns={columns} workers={workers} tolerance={tolerance}"
                        );
                    }
                })
                .unwrap();
            }
        }
    }
}

#[test]
fn direct_structural_budget_matches_the_incumbent_boundary() {
    let values =
        DenseMatrix::from_row_major(257, 2, (0..257).flat_map(|r| [1., r as f64]).collect())
            .unwrap();
    let design = BorrowedInterceptFeDesign::new(257, Vec::new()).unwrap();
    let required =
        BorrowedMapWorkspace::planned_payload_bytes(&design, 257, 2, rayon::current_num_threads())
            .unwrap();
    for short in [true, false] {
        let config = AbsorbConfig {
            solver_memory_budget: Some(FeSolverMemoryBudget {
                execution_bytes: 19,
                usable_bytes: 19 + required - u128::from(short),
            }),
            ..AbsorbConfig::default()
        };
        let actual = project(
            &design,
            values.clone().into(),
            RawMaxima {
                scales: &[1., 256.],
                constant: 0,
            },
            config,
        );
        let expected = project_direct_categorical(
            &design,
            BorrowedMapWeights::Unit,
            values.clone(),
            config.map,
            config.solver_memory_budget,
            true,
        );
        if short {
            assert_eq!(actual.unwrap_err(), expected.unwrap_err());
        } else {
            assert_eq!(
                actual.unwrap().report().solver_workspace_peak,
                expected.unwrap().report().solver_workspace_peak
            );
        }
    }
}

#[test]
fn direct_structural_cancels_both_fresh_statistics_passes() {
    const NAME: &str = "fe::borrowed_map::structural_direct::tests::direct_structural_cancels_both_fresh_statistics_passes";
    const CHILD: &str = "FEREG_STRUCTURAL_CANCEL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        assert!(std::process::Command::new(std::env::current_exe().unwrap())
            .env(CHILD, "1")
            .args([NAME, "--exact", "--test-threads=1"])
            .status()
            .unwrap()
            .success());
        return;
    }
    let values =
        DenseMatrix::from_row_major(4_096, 2, (0..4_096).flat_map(|r| [1., r as f64]).collect())
            .unwrap();
    let design = BorrowedInterceptFeDesign::new(4_096, Vec::new()).unwrap();
    let mut workspace =
        BorrowedMapWorkspace::new(&design, 4_096, 2, &mut SystemWorkspaceReservation).unwrap();
    crate::interrupt::test_polls::cancel_at(Some(1));
    assert_eq!(
        raw_statistics(&values, &[1., 4095.], &mut workspace)
            .unwrap_err()
            .code,
        ErrorCode::Interrupted
    );
    crate::interrupt::test_polls::cancel_at(None);
    raw_statistics(&values, &[1., 4095.], &mut workspace).unwrap();
    let mut residuals = values.clone();
    subtract_structural_constant(&mut residuals, &workspace.group_sums).unwrap();
    let weights = validate_weights(BorrowedMapWeights::Unit, values.nrows()).unwrap();
    update_workspace_norms(&residuals, weights, &mut workspace, false).unwrap();
    // The weight-sum scan makes four polls; stop on the first actual residual
    // moment row, ensuring the independent certificate itself observes Break.
    crate::interrupt::test_polls::cancel_at(Some(4));
    assert_eq!(
        structural_constant_means_with_scales(weights, &residuals, &mut workspace, true)
            .unwrap_err()
            .code,
        ErrorCode::Interrupted
    );
    crate::interrupt::test_polls::cancel_at(None);
    assert!(project(
        &design,
        values.into(),
        RawMaxima {
            scales: &[1., 4095.],
            constant: 0
        },
        AbsorbConfig::default()
    )
    .is_ok());
}
