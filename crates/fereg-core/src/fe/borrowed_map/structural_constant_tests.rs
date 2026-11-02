use super::*;

#[path = "../../../../fereg-testkit/src/structural_constant_reference.rs"]
pub(super) mod reference;

fn config() -> MapConfig {
    MapConfig {
        tolerance: 1e-11,
        max_iterations: 100,
        check_every: 1,
        symmetric: true,
    }
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn structural_constant_bands_preserve_scalar_arithmetic_and_projection() {
    for rows in [17, 16_385] {
        for width in [1, 3, 65, 130] {
            let data: Vec<_> = (0..rows)
                .flat_map(|row| {
                    (0..width).map(move |column| {
                        let signed = (row % 17) as f64 / 8.0 - 1.0;
                        match column % 6 {
                            0 => 1.0,
                            1 => signed,
                            2 => 1e12 + signed,
                            3 => 1e150 * signed,
                            4 => 1e-150 * signed,
                            _ => {
                                if row % 2 == 0 {
                                    -0.0
                                } else {
                                    0.0
                                }
                            }
                        }
                    })
                })
                .collect();
            let values = DenseMatrix::from_row_major(rows, width, data).unwrap();
            let supplied: Vec<_> = (0..rows)
                .map(|row| 2.0_f64.powi((row % 11) as i32 - 5))
                .collect();
            for provided in [false, true] {
                let weights = if provided {
                    BorrowedMapWeights::Provided(&supplied)
                } else {
                    BorrowedMapWeights::Unit
                };
                let (total, scales, means, corrections) = reference::scalar_means(
                    values.as_slice(),
                    width,
                    provided.then_some(supplied.as_slice()),
                );
                let expected: Vec<_> = values
                    .as_slice()
                    .iter()
                    .enumerate()
                    .map(|(index, value)| *value - means[index % width])
                    .collect();
                for workers in [1, 4, 16] {
                    crate::ThreadTeam::new(
                        crate::ThreadConfig::from_requested(workers, true).unwrap(),
                    )
                    .install(|| {
                        let design = BorrowedInterceptFeDesign::new(rows, Vec::new()).unwrap();
                        let mut workspace = BorrowedMapWorkspace::new(
                            &design,
                            rows,
                            width,
                            &mut SystemWorkspaceReservation,
                        )
                        .unwrap();
                        let actual_total = structural_constant_means(
                            validate_weights(weights, rows).unwrap(),
                            &values,
                            &mut workspace,
                        )
                        .unwrap();
                        assert_eq!(total.to_bits(), actual_total.to_bits());
                        assert_eq!(bits(&scales), bits(&workspace.value_scales));
                        assert_eq!(bits(&means), bits(&workspace.group_sums));
                        assert_eq!(bits(&corrections), bits(&workspace.group_corrections));
                        let result = absorb_with_reservation_mode(
                            &design,
                            weights,
                            values.clone(),
                            config(),
                            true,
                            false,
                            &mut SystemWorkspaceReservation,
                        )
                        .unwrap();
                        assert_eq!(bits(&expected), bits(result.residualized().as_slice()));
                        assert!(result.report().certified());
                        assert_eq!(result.report().iterations, 1);
                        let cg = absorb_cg_with_reservation(
                            &design,
                            weights,
                            values.clone(),
                            config(),
                            true,
                            CgContinuation::ORDINARY,
                            &mut SystemWorkspaceReservation,
                        )
                        .unwrap();
                        assert_eq!(bits(&expected), bits(cg.residualized().as_slice()));
                        assert!(cg.report().certified());
                        assert_eq!(cg.report().iterations, 1);
                    })
                    .unwrap();
                }
            }
        }
    }
}

#[test]
fn structural_constant_reuse_keeps_zero_iteration_shortcut() {
    let rows = 16_385;
    let design = BorrowedInterceptFeDesign::new(rows, Vec::new()).unwrap();
    let values = DenseMatrix::from_row_major(rows, 3, vec![0.0; rows * 3]).unwrap();
    for workers in [1, 4, 16] {
        crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
            .install(|| {
                let result = absorb_with_reservation_mode(
                    &design,
                    BorrowedMapWeights::Unit,
                    values.clone(),
                    config(),
                    true,
                    false,
                    &mut SystemWorkspaceReservation,
                )
                .unwrap();
                assert_eq!(result.report().iterations, 0);
                assert_eq!(result.residualized(), &values);
                assert!(result.report().certified());
                let cg = absorb_cg_with_reservation(
                    &design,
                    BorrowedMapWeights::Unit,
                    values.clone(),
                    config(),
                    true,
                    CgContinuation::ORDINARY,
                    &mut SystemWorkspaceReservation,
                )
                .unwrap();
                assert_eq!(cg.report().iterations, 0);
                assert_eq!(cg.residualized(), &values);
                assert!(cg.report().certified());
            })
            .unwrap();
    }
}
