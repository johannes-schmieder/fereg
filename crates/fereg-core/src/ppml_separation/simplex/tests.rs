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
    let (coordinates, _) = phase_one_cone(&cone, 1000, 1e-10)
        .expect("phase one")
        .expect("feasible");
    let values = cone
        .iter()
        .map(|row| row[0] * coordinates[0] + row[1] * coordinates[1])
        .collect::<Vec<_>>();
    assert!(values.iter().all(|value| *value >= -1e-8));
    assert!((values.iter().sum::<f64>() - 1.0).abs() < 1e-8);
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
    let error = nullspace::nullspace_basis(&x, &[1], 1e-10)
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
