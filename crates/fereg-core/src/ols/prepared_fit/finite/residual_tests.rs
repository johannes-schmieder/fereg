//! Exact witnesses against the two formerly separate physical traversals.
//! This fusion changes neither centered-FMA nor numeric raw-FMA arithmetic;
//! factor layout subtraction and the compensated original score stay separate.
use super::*;
use crate::ols::centered::{CenteredRows, CenteredSummary};

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}

fn separate_reference(
    values: CenteredRows<'_>,
    kept: &[usize],
    coefficients: &[Option<f64>],
) -> (Vec<f64>, Vec<f64>) {
    let mut within = Vec::with_capacity(values.rows());
    for row in 0..values.rows() {
        let mut value = values.response(row).unwrap();
        for &column in kept {
            value =
                (-coefficients[column].unwrap()).mul_add(values.value(row, column).unwrap(), value);
        }
        within.push(value);
    }
    let mut raw: Vec<_> = (0..values.rows())
        .map(|row| values.raw()[(row, 0)])
        .collect();
    // Same production physical kernel as NumericOlsColumns::subtract_fitted,
    // with columns outermost, independently of the fused observation loop.
    for (column, coefficient) in coefficients.iter().enumerate() {
        if let Some(coefficient) = coefficient {
            let source: Vec<_> = (0..values.rows())
                .map(|row| values.raw()[(row, column + 1)])
                .collect();
            crate::kernels::coefficient_add(-coefficient, Some(&source), &mut raw);
        }
    }
    (within, raw)
}

fn assert_exact(raw: &DenseMatrix, kept: &[usize], coefficients: &[Option<f64>]) {
    let summary = CenteredSummary::build(raw, &mut SystemFitReservation).unwrap();
    let values = CenteredRows::new(raw, &summary).unwrap();
    let (within, original) = separate_reference(values, kept, coefficients);
    let (actual_within, actual_original) =
        centered_residuals(values, kept, coefficients, true, &mut SystemFitReservation).unwrap();
    assert_eq!(bits(&actual_within), bits(&within));
    assert_eq!(bits(&actual_original.unwrap()), bits(&original));
    let (unfused, original) =
        centered_residuals(values, kept, coefficients, false, &mut SystemFitReservation).unwrap();
    assert_eq!(bits(&unfused), bits(&within));
    assert!(original.is_none());
}

#[test]
fn fused_centered_and_raw_residuals_are_exact_at_offsets_and_extremes() {
    let tiny = f64::from_bits(1);
    for (rows, scale, offset) in [
        (17, 1.0, 0.0),
        (513, 1.0, 2.0_f64.powi(50)),
        (17, 1e150, 0.0),
        (17, 1e-200, 0.0),
        (17, tiny, 0.0),
    ] {
        let raw = DenseMatrix::from_row_major(
            rows,
            4,
            (0..rows)
                .flat_map(|row| {
                    let x = (row % 7) as f64 - 3.0;
                    let z = (row % 5) as f64 - 2.0;
                    [offset + (x + z) * scale, offset + x * scale, z * scale, 1.0]
                })
                .collect(),
        )
        .unwrap();
        assert_exact(&raw, &[0, 1], &[Some(0.25), Some(-0.75), None]);
    }
}

#[test]
fn signed_zero_zero_coefficients_and_empty_basis_preserve_bits() {
    let raw = DenseMatrix::from_row_major(
        4,
        4,
        vec![
            -0.0, 0.0, -0.0, 1.0, 0.0, -0.0, 0.0, 1.0, -0.0, -0.0, 0.0, 1.0, 0.0, 0.0, -0.0, 1.0,
        ],
    )
    .unwrap();
    assert_exact(&raw, &[0, 1], &[Some(-0.0), Some(-2.0), None]);
    assert_exact(&raw, &[0, 1], &[Some(0.0), Some(2.0), None]);
    assert_exact(&raw, &[], &[None, None, None]);
}

#[test]
fn fused_residuals_repeat_exactly_across_workers() {
    let rows = 32_769;
    let raw = DenseMatrix::from_row_major(
        rows,
        4,
        (0..rows)
            .flat_map(|row| {
                let x = (row % 17) as f64 - 8.0;
                let z = (row % 13) as f64 - 6.0;
                [2.0_f64.powi(40) + x + z, x, z, 1.0]
            })
            .collect(),
    )
    .unwrap();
    for workers in [1, 4, 16] {
        crate::parallel::ThreadTeam::new(
            crate::options::ThreadConfig::from_requested(workers, false).unwrap(),
        )
        .install(|| assert_exact(&raw, &[0, 1], &[Some(0.75), Some(-0.5), None]))
        .unwrap();
    }
}

#[test]
fn centered_error_precedes_raw_error_and_raw_overflow_still_fails() {
    let raw = DenseMatrix::from_row_major(2, 2, vec![1.0, 2.0, -1.0, -2.0]).unwrap();
    let summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let error = centered_residuals(
        CenteredRows::new(&raw, &summary).unwrap(),
        &[0],
        &[Some(f64::MAX)],
        true,
        &mut SystemFitReservation,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidData);
    assert_eq!(
        error.message,
        "centered original-source residual is not finite"
    );
    let raw = DenseMatrix::from_row_major(1, 2, vec![-f64::MAX, 2.0]).unwrap();
    let summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let (within, original) = centered_residuals(
        CenteredRows::new(&raw, &summary).unwrap(),
        &[0],
        &[Some(f64::MAX)],
        true,
        &mut SystemFitReservation,
    )
    .unwrap();
    assert!(within.iter().all(|value| value.is_finite()));
    // The handoff preserves overflow until reported_bread's existing failure
    // boundary, whose injection-specific fallback rules remain authoritative.
    let error = validate_raw_residuals(&original.unwrap(), "raw-only failure").unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidData);
    assert_eq!(error.message, "raw-only failure");
}

#[derive(Default)]
struct Reservation {
    calls: usize,
    bytes: usize,
    fail: Option<usize>,
}
impl FitReservation for Reservation {
    fn reserve<T>(&mut self, target: &mut Vec<T>, count: usize, label: &'static str) -> Result<()> {
        let call = self.calls;
        self.calls += 1;
        if self.fail == Some(call) {
            return Err(allocation("residual reservation denied"));
        }
        target
            .try_reserve_exact(count)
            .map_err(|_| allocation(label))?;
        self.bytes += target.capacity() * std::mem::size_of::<T>();
        Ok(())
    }
}

#[test]
fn residual_pair_uses_two_fallible_vectors_without_additional_row_storage() {
    let raw = DenseMatrix::from_row_major(3, 2, vec![2.0, 1.0, 4.0, 2.0, 6.0, 3.0]).unwrap();
    let summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let values = CenteredRows::new(&raw, &summary).unwrap();
    let mut reservation = Reservation::default();
    centered_residuals(values, &[0], &[Some(2.0)], true, &mut reservation).unwrap();
    assert_eq!(reservation.calls, 2);
    assert_eq!(
        reservation.bytes,
        2 * raw.nrows() * std::mem::size_of::<f64>()
    );
    for fail in [0, 1] {
        let mut reservation = Reservation {
            fail: Some(fail),
            ..Default::default()
        };
        assert_eq!(
            centered_residuals(values, &[0], &[Some(2.0)], true, &mut reservation)
                .unwrap_err()
                .code,
            ErrorCode::Allocation
        );
    }
}

#[test]
fn fused_residual_cancellation_isolated() {
    const KEY: &str = "FEREG_FUSED_CENTERED_RESIDUAL_CHILD";
    if std::env::var_os(KEY).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ols::prepared_fit::finite::residual_tests::fused_residual_cancellation_isolated",
                "--nocapture",
            ])
            .env(KEY, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let rows = 32_769;
    let raw = DenseMatrix::from_row_major(
        rows,
        2,
        (0..rows)
            .flat_map(|row| [(row % 17) as f64, (row % 13) as f64])
            .collect(),
    )
    .unwrap();
    let summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    for stop in [0, 2] {
        crate::interrupt::test_polls::cancel_at(Some(stop));
        let error = centered_residuals(
            CenteredRows::new(&raw, &summary).unwrap(),
            &[0],
            &[Some(0.5)],
            true,
            &mut SystemFitReservation,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Interrupted);
    }
    crate::interrupt::test_polls::cancel_at(None);
}
