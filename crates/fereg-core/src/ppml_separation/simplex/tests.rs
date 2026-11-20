use super::*;

#[test]
fn finds_two_regressor_certificate_from_primer() {
    let outcome = vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0];
    let x = DenseMatrix::from_columns(&[
        vec![2.0, -1.0, 0.0, 0.0, 5.0, 6.0],
        vec![-1.0, 2.0, 0.0, 0.0, -10.0, -12.0],
    ])
    .expect("matrix");
    let result = model_simplex(&outcome, &x, 1e-10, 1e-8, 1000, 1_000_000).expect("simplex");
    assert_eq!(result.status, MethodStatus::Certified);
    assert_eq!(result.dropped, vec![0]);
    assert!(result.certificate.expect("certificate").passed);
}

#[test]
fn rejects_full_rank_positive_sample() {
    let outcome = vec![0.0, 1.0, 2.0];
    let x = DenseMatrix::from_columns(&[vec![1.0, 1.0, 1.0]]).expect("matrix");
    let result = model_simplex(&outcome, &x, 1e-10, 1e-8, 100, 100_000).expect("simplex");
    assert_eq!(result.status, MethodStatus::NoSeparation);
}

#[test]
fn reservation_failure_maps_to_allocation() {
    let outcome = vec![0.0, 1.0, 2.0];
    let x = DenseMatrix::from_columns(&[vec![1.0, 1.0, 1.0]]).expect("matrix");
    super::inject_reservation_failure(0);
    let error = model_simplex(&outcome, &x, 1e-10, 1e-8, 100, 100_000)
        .expect_err("injected simplex allocation failure");
    assert_eq!(error.code, crate::error::ErrorCode::Allocation);
    assert_eq!(error.return_code(), 909);
}

#[test]
fn phase_one_combines_nullspace_directions() {
    // No single direction is nonnegative, but their sum is.
    let cone = vec![vec![2.0, -1.0], vec![-1.0, 2.0]];
    let PhaseOneConeOutcome::Feasible { coordinates, .. } =
        phase_one_cone(&cone, 1000, 1e-10).expect("phase one")
    else {
        panic!("cone must be feasible");
    };
    let values = cone
        .iter()
        .map(|row| row[0] * coordinates[0] + row[1] * coordinates[1])
        .collect::<Vec<_>>();
    assert!(values.iter().all(|value| *value >= -1e-8));
    assert!((values.iter().sum::<f64>() - 1.0).abs() < 1e-8);
}

#[test]
fn simplex_alone_certifies_nontrivial_infeasibility() {
    // The positive row leaves the second coefficient unidentified.  On the
    // two boundary rows it has opposite signs, so the only nonnegative cone
    // direction is zero and the unit-slack normalization is infeasible.
    let outcome = vec![1.0, 0.0, 0.0];
    let x =
        DenseMatrix::from_columns(&[vec![1.0, 1.0, 1.0], vec![0.0, 1.0, -1.0]]).expect("matrix");
    let result =
        model_simplex(&outcome, &x, 1e-10, 1e-8, 1000, 1_000_000).expect("simplex dual closure");
    assert_eq!(result.status, MethodStatus::NoSeparation);
    let certificate = result.certificate.expect("Farkas certificate");
    assert!(matches!(
        certificate.proof,
        SeparationCertificateProof::SimplexDualInfeasibility {
            boundary_rows: 2,
            nullity: 1,
        }
    ));
    assert!(certificate.passed);
    assert!(certificate.negative_margin_relative > 0.0);
}

#[test]
fn degenerate_duplicate_constraints_still_produce_a_verified_dual() {
    // Duplicate opposite constraints force degenerate zero-RHS pivots. Bland
    // ordering must still terminate with a dual that verifies against every
    // original (including duplicate) row.
    let cone = vec![vec![1.0], vec![-1.0], vec![1.0], vec![-1.0]];
    let PhaseOneConeOutcome::Infeasible { dual, iterations } =
        phase_one_cone(&cone, 1000, 1e-10).expect("degenerate phase one")
    else {
        panic!("opposite duplicate rays cannot satisfy unit normalization");
    };
    assert!(iterations < 1000);
    let certificate = verify_phase_one_farkas_certificate(&cone, &dual, 1e-8)
        .expect("dual verification execution")
        .expect("independently verified degenerate dual");
    assert!(certificate.passed);

    let mut forged = dual;
    *forged.last_mut().expect("normalization multiplier") = 1.0;
    assert!(verify_phase_one_farkas_certificate(&cone, &forged, 1e-8)
        .expect("forged verification execution")
        .is_none());
}

#[test]
fn injected_detector_reservation_failure_is_allocation_error() {
    let outcome = vec![0.0, 1.0];
    let x = DenseMatrix::from_columns(&[vec![1.0, 1.0]]).expect("matrix");
    inject_reservation_failure(0);
    let error = model_simplex(&outcome, &x, 1e-10, 1e-8, 100, 100_000)
        .expect_err("injected detector allocation failure");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn injected_nullspace_reservation_failure_is_allocation_error() {
    let x = DenseMatrix::from_columns(&[vec![1.0, 1.0]]).expect("matrix");
    inject_reservation_failure(0);
    let error = nullspace::nullspace_basis(&x, &mut [1], 1e-10, &mut || Ok(()))
        .expect_err("injected null-space allocation failure");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn injected_tableau_reservation_failure_is_allocation_error() {
    inject_reservation_failure(0);
    let error =
        phase_one_cone(&[vec![1.0]], 100, 1e-10).expect_err("injected tableau allocation failure");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn combined_peak_bound_covers_live_nested_kernel_storage() {
    // N=10, P=6, B=4, K=3. The phase-I branch dominates:
    // design 30 + basis/cone/tableau/terminal scratch 155 = 185 cells.
    assert_eq!(simplex_workspace_peak_cells(10, 6, 4, 3).unwrap(), 185);
    let overflow =
        simplex_workspace_peak_cells(usize::MAX, 1, 1, 2).expect_err("overflowing peak dimensions");
    assert_eq!(overflow.code, ErrorCode::Allocation);
}

#[test]
fn tiny_boundary_coordinate_still_reveals_an_exact_recession_direction() {
    for scale in [1., 2f64.powi(-40)] {
        let x = DenseMatrix::from_columns(&[vec![1., 1., 1.], vec![0., 0., scale]]).unwrap();
        let result = model_simplex(&[1., 2., 0.], &x, 1e-10, 1e-8, 100, 100_000).unwrap();
        assert_eq!(result.status, MethodStatus::Certified);
        assert_eq!(result.dropped, [2]);
        // A small floating-point Farkas residual cannot establish a dual of
        // the raw design when every strictly positive boundary weight has a
        // nonzero product with its unique nonzero coordinate.
        assert!(
            !exact_terminal::no_recession(&x, &[1., 2., 0.], &[-1.], 100_000, 100_000, &[0])
                .unwrap()
        );
    }
}

#[test]
fn rounded_pivot_is_not_a_positive_outcome_rank_proof() {
    // The exact positive rows (1,3) and (3,9) are collinear. Floating point
    // column normalization generates a spurious 2^-54 pivot in this system.
    let x = DenseMatrix::from_columns(&[vec![1., 3., 0.], vec![3., 9., 1.]]).unwrap();
    let y = [1., 2., 0.];
    assert!(!exact_terminal::full_positive_rank(&x, &y, &[0, 1], 100_000).unwrap());
    let result = model_simplex(&y, &x, 1e-18, 1e-8, 100, 100_000).unwrap();
    assert_ne!(result.status, MethodStatus::NoSeparation);
}

#[test]
fn dual_has_strict_positive_weights_for_unequal_opposing_rays_and_raw_moments() {
    let x = DenseMatrix::from_columns(&[vec![1., 1., 1.], vec![0., 2., -3.]]).unwrap();
    assert!(
        exact_terminal::no_recession(&x, &[1., 0., 0.], &[-1., -1.], 100_000, 100_000, &[0])
            .unwrap()
    );
    assert!(
        !exact_terminal::no_recession(&x, &[1., 0., 0.], &[-1., -1.], 1, 100_000, &[0]).unwrap()
    );
}

#[test]
fn positive_pivot_order_keeps_late_independent_rows_within_work_budget() {
    let mut x = vec![0.; 100];
    x[99] = 1.;
    let design = DenseMatrix::from_columns(&[vec![1.; 100], x]).unwrap();
    let mut outcome = vec![1.; 100];
    outcome[50] = 0.;
    assert!(exact_terminal::full_positive_rank(&design, &outcome, &[0, 99], 6).unwrap());
    // Nominating a dependent second pivot must never issue a certificate.
    assert!(!exact_terminal::full_positive_rank(&design, &outcome, &[0, 1], 6).unwrap());
}
