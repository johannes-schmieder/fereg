use super::*;
use crate::error::ErrorCode;
use crate::ols::prepared_fit::SystemFitReservation;

fn matrix(rows: usize, columns: usize, values: Vec<f64>) -> DenseMatrix {
    DenseMatrix::from_row_major(rows, columns, values).unwrap()
}

fn summary(raw: &DenseMatrix) -> CenteredSummary {
    CenteredSummary::build(raw, &mut SystemFitReservation).unwrap()
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual:e}, expected={expected:e}, tolerance={tolerance:e}"
    );
}

#[test]
fn rational_center_below_raw_ulp_has_independent_analytic_witness() {
    let base = 2.0_f64.powi(50);
    let raw = matrix(
        3,
        3,
        vec![
            base,
            base,
            7.0,
            base + 1.0,
            base + 1.0,
            7.0,
            base + 1.0,
            base + 1.0,
            7.0,
        ],
    );
    let stats = summary(&raw);
    let view = CenteredRows::new(&raw, &stats).unwrap();
    for (row, expected) in [-2.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0].into_iter().enumerate() {
        close(view.response(row).unwrap(), expected, 2.0 * f64::EPSILON);
        close(view.value(row, 0).unwrap(), expected, 2.0 * f64::EPSILON);
        assert_eq!(view.value(row, 1).unwrap(), 0.0);
        let mut output = [0.0; 2];
        view.fill_row(row, &mut output).unwrap();
        assert_eq!(output, [view.value(row, 0).unwrap(), 0.0]);
    }
    close(
        stats.within_norms()[0],
        (2.0_f64 / 3.0).sqrt(),
        4.0 * f64::EPSILON,
    );
    assert_eq!(stats.means()[0], base + 0.75);
    close(stats.centers_lo[0], -1.0 / 12.0, 2.0 * f64::EPSILON);
    assert_eq!(stats.centers_hi[0], stats.means()[0]);
    assert_eq!(stats.maxima()[0], base + 1.0);
    assert!(stats.centered_relative_moments()[0] < 1e-30);
    assert!(std::ptr::eq(view.summary(), &stats));
    assert!(std::ptr::eq(view.raw(), &raw));
}

#[test]
fn exponent_extremes_and_subnormal_residual_certificate() {
    for exponent in [-1000, 0, 900, 1022] {
        let value = 2.0_f64.powi(exponent);
        let raw = matrix(2, 1, vec![value, -value]);
        let stats = summary(&raw);
        let view = CenteredRows::new(&raw, &stats).unwrap();
        assert_eq!(stats.means()[0], 0.0);
        assert_eq!(view.response(0).unwrap(), value);
        assert_eq!(view.response(1).unwrap(), -value);
        close(
            stats.raw_norms()[0] / value,
            2.0_f64.sqrt(),
            2.0 * f64::EPSILON,
        );
        assert_eq!(stats.within_norms()[0], stats.raw_norms()[0]);
        assert_eq!(stats.centered_relative_moments()[0], 0.0);
    }
    let smallest = f64::from_bits(1);
    let raw = matrix(2, 1, vec![0.0, smallest]);
    let stats = summary(&raw);
    // The exact mean is half a minimum subnormal. Its rounded physical value
    // vanishes, but the actual evaluated row values do not sum to zero.
    assert_eq!(stats.centered_means()[0], 0.0);
    close(
        stats.centered_relative_moments()[0],
        0.5 * 2.0_f64.sqrt(),
        f64::EPSILON,
    );
    let raw = matrix(2, 2, vec![0.0, -0.0, -0.0, 0.0]);
    let stats = summary(&raw);
    assert_eq!(stats.raw_norms(), &[0.0, 0.0]);
    assert_eq!(stats.within_norms(), &[0.0, 0.0]);
    assert_eq!(stats.centered_relative_moments(), &[0.0, 0.0]);
}

#[test]
fn unsafe_exponent_spread_or_nonfinite_data_declines() {
    for values in [
        vec![2.0_f64.powi(900), -2.0_f64.powi(900), 1e-300],
        vec![1.0, f64::NAN, 2.0],
        vec![1.0, f64::INFINITY, 2.0],
        vec![f64::MAX, f64::MAX, f64::MAX],
    ] {
        let raw = matrix(3, 1, values);
        assert_eq!(
            CenteredSummary::build(&raw, &mut SystemFitReservation)
                .unwrap_err()
                .code,
            ErrorCode::InvalidData
        );
    }
}

fn bits(stats: &CenteredSummary) -> Vec<u64> {
    [
        &stats.raw_norms,
        &stats.maxima,
        &stats.minimum_values,
        &stats.maximum_values,
        &stats.means,
        &stats.centers_hi,
        &stats.centers_lo,
        &stats.within_norms,
        &stats.centered_means,
        &stats.centered_relative_moments,
        &stats.units,
    ]
    .into_iter()
    .flat_map(|values| values.iter().map(|value| value.to_bits()))
    .collect()
}

#[test]
fn shape_only_leaves_repeat_exactly_across_worker_counts() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    let rows = 32_769;
    let columns = 5;
    let raw = matrix(
        rows,
        columns,
        (0..rows)
            .flat_map(|row| {
                let offset = (row % 13) as f64 - 6.0;
                [
                    2.0_f64.powi(45) + offset,
                    offset,
                    offset * 1e200,
                    offset * 1e-200,
                    4.0,
                ]
            })
            .collect(),
    );
    let baseline = summary(&raw);
    for workers in [1, 2, 4, 16] {
        let team = crate::parallel::ThreadTeam::new(
            crate::options::ThreadConfig::from_requested(workers, false).unwrap(),
        );
        let actual = team.install(|| summary(&raw)).unwrap();
        assert_eq!(bits(&actual), bits(&baseline));
        let view = CenteredRows::new(&raw, &actual).unwrap();
        let reference = CenteredRows::new(&raw, &baseline).unwrap();
        for row in [0, 16_383, 16_384, rows - 1] {
            for column in 0..columns - 1 {
                assert_eq!(
                    view.value(row, column).unwrap().to_bits(),
                    reference.value(row, column).unwrap().to_bits()
                );
            }
        }
    }
}

#[test]
fn borrowed_view_rejects_different_owner_and_bad_shapes() {
    let raw = matrix(2, 2, vec![1.0, 4.0, 3.0, 6.0]);
    let stats = summary(&raw);
    let other = raw.clone();
    assert!(CenteredRows::new(&other, &stats).is_err());
    let view = CenteredRows::new(&raw, &stats).unwrap();
    assert_eq!(view.rows(), 2);
    assert_eq!(view.columns(), 1);
    assert!(view.response(2).is_err());
    assert!(view.value(0, 1).is_err());
    assert!(view.fill_row(0, &mut []).is_err());
    assert!(CenteredSummary::workspace_bytes(0, 2).is_err());
    assert!(CenteredSummary::workspace_bytes(2, 0).is_err());
    assert!(CenteredSummary::workspace_bytes(usize::MAX, 2).is_err());
}

#[derive(Default)]
struct RecordingReservation {
    calls: usize,
    bytes: u128,
    fail: Option<usize>,
}

impl FitReservation for RecordingReservation {
    fn reserve<T>(&mut self, target: &mut Vec<T>, count: usize, label: &'static str) -> Result<()> {
        let call = self.calls;
        self.calls += 1;
        if self.fail == Some(call) {
            return Err(allocation("injected summary allocation failure"));
        }
        target
            .try_reserve_exact(count)
            .map_err(|_| allocation(label))?;
        self.bytes += target.capacity() as u128 * std::mem::size_of::<T>() as u128;
        Ok(())
    }
}

#[test]
fn all_allocations_are_fallible_and_peak_declaration_includes_partial_bank() {
    let raw = matrix(17, 3, (0..51).map(|value| value as f64).collect());
    let mut reservation = RecordingReservation::default();
    let stats = CenteredSummary::build(&raw, &mut reservation).unwrap();
    assert_eq!(reservation.calls, RESULT_VECTORS + 1);
    assert_eq!(
        reservation.bytes,
        CenteredSummary::workspace_bytes(17, 3).unwrap()
    );
    assert_eq!(stats.resident_bytes(), (RESULT_VECTORS * 3 * 8) as u128);
    for fail in 0..reservation.calls {
        let mut denied = RecordingReservation {
            fail: Some(fail),
            ..Default::default()
        };
        assert_eq!(
            CenteredSummary::build(&raw, &mut denied).unwrap_err().code,
            ErrorCode::Allocation
        );
    }
    let maximum = CenteredSummary::workspace_bytes(1_000_001, 65).unwrap();
    let beyond = CenteredSummary::workspace_bytes(8_000_001, 65).unwrap();
    assert!(maximum <= beyond);
    assert_eq!(
        beyond,
        (65 * (RESULT_VECTORS * 8 + MAX_LEAVES * 32)) as u128
    );
}

#[test]
fn cancellation_in_summary_scan_isolated() {
    const KEY: &str = "FEREG_CENTERED_SUMMARY_CANCEL_CHILD";
    if std::env::var_os(KEY).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ols::centered::tests::cancellation_in_summary_scan_isolated",
                "--nocapture",
            ])
            .env(KEY, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let raw = matrix(
        32_769,
        2,
        (0..65_538).map(|value| (value % 19) as f64).collect(),
    );
    for stop in [0, 4, 40, 75] {
        crate::interrupt::test_polls::cancel_at(Some(stop));
        let error = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap_err();
        assert_eq!(error.code, ErrorCode::Interrupted);
    }
    crate::interrupt::test_polls::cancel_at(None);
}

#[test]
fn observed_extrema_preserve_normal_subnormal_adjacent_and_constant_values() {
    let tiny = f64::from_bits(1);
    let adjacent = f64::from_bits(1.0_f64.to_bits() + 1);
    let raw = matrix(
        3,
        5,
        vec![
            -4.0, -tiny, 1.0, -7.0, 0.0, 3.0, 0.0, adjacent, -7.0, -0.0, 1.0, tiny, 1.0, -7.0, 0.0,
        ],
    );
    let stats = summary(&raw);
    assert_eq!(stats.minimum_values(), &[-4.0, -tiny, 1.0, -7.0, 0.0]);
    assert_eq!(stats.maximum_values(), &[3.0, tiny, adjacent, -7.0, 0.0]);
    assert_eq!(stats.maxima(), &[4.0, tiny, adjacent, 7.0, 0.0]);
    assert_eq!(std::mem::size_of::<Partial>(), 32);
    assert_eq!(stats.resident_bytes(), 11 * 5 * 8);
}

#[test]
fn observed_extrema_merge_partial_leaves_without_zero_bias() {
    let rows = 32_769;
    let raw = matrix(
        rows,
        2,
        (0..rows)
            .flat_map(|row| {
                let positive = if row == rows - 1 { 11.0 } else { 2.0 };
                let negative = if row == 16_384 { -13.0 } else { -3.0 };
                [positive, negative]
            })
            .collect(),
    );
    let stats = summary(&raw);
    assert_eq!(stats.minimum_values(), &[2.0, -13.0]);
    assert_eq!(stats.maximum_values(), &[11.0, -3.0]);
}
