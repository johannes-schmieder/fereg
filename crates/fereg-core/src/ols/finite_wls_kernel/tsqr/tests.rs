use super::*;
use crate::data::DenseMatrix;
use crate::error::ErrorCode;
use crate::ols::centered::{CenteredRows, CenteredSummary};
use crate::ols::prepared_fit::SystemFitReservation;

fn fixture(rows: usize, perturbation: f64, response_scale: f64) -> DenseMatrix {
    let mut values = Vec::with_capacity(rows * 3);
    for row in 0..rows {
        let x = (row % 17) as f64 - 8.0;
        let independent = ((row * 7) % 23) as f64 - 11.0;
        let z = if perturbation == 0.0 {
            independent
        } else {
            x + perturbation * independent
        };
        values.extend_from_slice(&[(2.0 * x - 3.0 * z + 5.0) * response_scale, x, z]);
    }
    DenseMatrix::from_row_major(rows, 3, values).unwrap()
}

fn source<'a>(raw: &'a DenseMatrix, stats: &'a CenteredSummary) -> FixedBlockMomentSource<'a> {
    FixedBlockMomentSource::Centered {
        values: CenteredRows::new(raw, stats).unwrap(),
        selected_columns: None,
        column_scales: Some(&stats.within_norms()[1..]),
    }
}

fn run(source: FixedBlockMomentSource<'_>) -> (Vec<f64>, Vec<f64>) {
    source.validate().unwrap();
    let n = source.dimension();
    let stored = match source {
        FixedBlockMomentSource::Centered { values, .. } => values.columns(),
        _ => unreachable!(),
    };
    let mut bank = vec![0.0; workspace_elements(source.rows(), n, stored).unwrap()];
    let mut qr = vec![0.0; n * n];
    let mut rhs = vec![0.0; n];
    compress(source, &mut bank, &mut qr, &mut rhs).unwrap();
    (qr, rhs)
}

fn close(actual: f64, expected: f64, scale: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * scale,
        "actual={actual:e}, expected={expected:e}, scale={scale:e}"
    );
}

// Independent observation-space products. No compression internals or root
// factor are used to form this witness, and every scalar sum is compensated.
fn witness(view: CenteredRows<'_>, qr: &[f64], rhs: &[f64]) {
    let n = view.columns();
    let scales = &view.summary().within_norms()[1..];
    for column in 0..n {
        for other in column..n {
            let mut expected = 0.0;
            let mut correction = 0.0;
            for row in 0..view.rows() {
                neumaier_add(
                    view.value(row, column).unwrap() / scales[column]
                        * (view.value(row, other).unwrap() / scales[other]),
                    &mut expected,
                    &mut correction,
                );
            }
            let actual: f64 = (0..n)
                .map(|row| qr[row * n + column] * qr[row * n + other])
                .sum();
            close(actual, expected + correction, 1.0, 8e-12);
        }
        let response_norm = view.summary().raw_norms()[0];
        let mut expected = 0.0;
        let mut correction = 0.0;
        for row in 0..view.rows() {
            neumaier_add(
                view.value(row, column).unwrap() / scales[column]
                    * (view.response(row).unwrap() / response_norm),
                &mut expected,
                &mut correction,
            );
        }
        let actual: f64 = (0..n)
            .map(|row| qr[row * n + column] * (rhs[row] / response_norm))
            .sum();
        close(actual, expected + correction, 1.0, 8e-12);
    }
}

#[test]
fn analytic_full_rank_solution_and_independent_products() {
    for rows in [5, 17, 257, 513, 32_769] {
        let raw = fixture(rows, 0.0, 1.0);
        let stats = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
        let view = CenteredRows::new(&raw, &stats).unwrap();
        let (qr, rhs) = run(source(&raw, &stats));
        witness(view, &qr, &rhs);
        let second = rhs[1] / qr[3];
        let first = (rhs[0] - qr[1] * second) / qr[0];
        close(first / stats.within_norms()[1], 2.0, 1.0, 1e-11);
        close(second / stats.within_norms()[2], -3.0, 1.0, 1e-11);
        assert_eq!(qr[2], 0.0);
    }
}

#[test]
fn nearly_collinear_solution_retains_observation_space_information() {
    let raw = fixture(32_769, 2.0_f64.powi(-24), 1.0);
    let stats = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let (qr, rhs) = run(source(&raw, &stats));
    witness(CenteredRows::new(&raw, &stats).unwrap(), &qr, &rhs);
    let second = rhs[1] / qr[3];
    let first = (rhs[0] - qr[1] * second) / qr[0];
    close(first / stats.within_norms()[1], 2.0, 1.0, 2e-7);
    close(second / stats.within_norms()[2], -3.0, 1.0, 2e-7);
    assert!(qr[3].abs() > 1e-8 && qr[3].abs() < 1e-6);
}

#[test]
fn extreme_response_and_exact_dependency_are_finite() {
    let raw = fixture(513, 0.0, 1e300);
    let stats = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let (qr, rhs) = run(source(&raw, &stats));
    witness(CenteredRows::new(&raw, &stats).unwrap(), &qr, &rhs);
    assert!(rhs.iter().all(|value| value.is_finite()));
    let raw = DenseMatrix::from_row_major(
        513,
        3,
        (0..513)
            .flat_map(|row| {
                let x = (row % 19) as f64;
                [3.0 * x + 7.0, x, 2.0 * x]
            })
            .collect(),
    )
    .unwrap();
    let stats = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let (qr, rhs) = run(source(&raw, &stats));
    witness(CenteredRows::new(&raw, &stats).unwrap(), &qr, &rhs);
    assert!(qr[3].abs() < 1e-13);
}

#[test]
fn fixed_leaf_and_tree_order_is_exact_across_workers() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let raw = fixture(32_769, 0.0, 1.0);
    let stats = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let source = source(&raw, &stats);
    let baseline = run(source);
    let expected: Vec<_> = baseline
        .0
        .iter()
        .chain(&baseline.1)
        .map(|v| v.to_bits())
        .collect();
    for workers in [1, 2, 4, 16] {
        let team = crate::parallel::ThreadTeam::new(
            crate::options::ThreadConfig::from_requested(workers, false).unwrap(),
        );
        let actual = team.install(|| run(source)).unwrap();
        assert_eq!(
            actual
                .0
                .iter()
                .chain(&actual.1)
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn every_selected_rank_fits_admission_envelope() {
    for rows in [1, 16_385, 1_000_001] {
        for stored in [1, 16, 64, 65, 129] {
            let envelope = workspace_envelope_bytes(rows, stored, stored).unwrap();
            for rank in 1..=stored {
                let elements = workspace_elements(rows, rank, stored).unwrap();
                assert!(elements as u128 * 8 <= envelope);
            }
        }
    }
    assert!(workspace_elements(0, 2, 2).is_err());
    assert!(workspace_elements(2, 0, 2).is_err());
    assert!(workspace_elements(2, 3, 2).is_err());
    assert!(workspace_elements(2, usize::MAX, usize::MAX).is_err());
    let raw = fixture(17, 0.0, 1.0);
    let stats = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    assert_eq!(
        compress(source(&raw, &stats), &mut [], &mut [0.0; 4], &mut [0.0; 2])
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn panel_and_merge_cancellation_isolated() {
    const KEY: &str = "FEREG_TSQR_CANCEL_CHILD";
    if std::env::var_os(KEY).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ols::finite_wls_kernel::tsqr::tests::panel_and_merge_cancellation_isolated",
                "--nocapture",
            ])
            .env(KEY, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let raw = fixture(32_769, 0.0, 1.0);
    let stats = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let source = source(&raw, &stats);
    let mut bank = vec![0.0; workspace_elements(source.rows(), 2, 2).unwrap()];
    for stop in [0, 10, 100] {
        crate::interrupt::test_polls::cancel_at(Some(stop));
        let error = compress(source, &mut bank, &mut [0.0; 4], &mut [0.0; 2]).unwrap_err();
        assert_eq!(error.code, ErrorCode::Interrupted);
    }
    crate::interrupt::test_polls::cancel_at(None);
    let shape = Shape::new(32_769, 2, 2).unwrap();
    for stop in [0, 1] {
        let mut left = vec![0.0; shape.stride];
        let mut right = vec![0.0; shape.stride];
        for column in 0..2 {
            left[column * shape.height + column] = 1.0;
            right[column * shape.height + column] = 1.0;
        }
        crate::interrupt::test_polls::cancel_at(Some(stop));
        assert_eq!(
            merge(shape, &mut left, &right).unwrap_err().code,
            ErrorCode::Interrupted
        );
        crate::interrupt::test_polls::cancel_at(None);
    }
}

#[test]
fn one_column_panels_and_partial_leaves_have_analytic_solution() {
    let raw = DenseMatrix::from_row_major(
        32_769,
        2,
        (0..32_769)
            .flat_map(|row| {
                let x = (row % 13) as f64 - 6.0;
                [2.0 * x + 5.0, x]
            })
            .collect(),
    )
    .unwrap();
    let stats = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let (qr, rhs) = run(source(&raw, &stats));
    witness(CenteredRows::new(&raw, &stats).unwrap(), &qr, &rhs);
    close(qr[0].abs(), 1.0, 1.0, 5e-13);
    close(rhs[0] / qr[0] / stats.within_norms()[1], 2.0, 1.0, 5e-13);
}
