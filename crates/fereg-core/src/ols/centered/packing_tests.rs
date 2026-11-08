use super::*;
use crate::ols::prepared_fit::SystemFitReservation;

fn scalar_panel(
    view: CenteredRows<'_>,
    first: usize,
    rows: usize,
    selected: &[usize],
    scales: &[f64],
) -> Result<Vec<f64>> {
    let mut panel = vec![0.; rows * (selected.len() + 1)];
    let mut row = vec![0.; view.columns()];
    for offset in 0..rows {
        view.fill_row(first + offset, &mut row)?;
        for (column, &source) in selected.iter().enumerate() {
            panel[column * rows + offset] = finite(
                row[source] / scales[column],
                "finite-WLS standardized column is not finite",
            )?;
        }
        panel[selected.len() * rows + offset] = view.response(first + offset)? / 2.;
    }
    Ok(panel)
}

#[test]
fn bulk_panels_match_scalar_bits_for_offsets_scales_selection_and_tails() {
    let _stable_route = crate::fe::block_schur::stable_route_for_test();
    for columns in [2, 3, 8, 17, 66] {
        let raw = DenseMatrix::from_row_major(
            513,
            columns,
            (0..513)
                .flat_map(|row| {
                    (0..columns).map(move |column| {
                        let v = ((row * 13 + column * 7) % 19) as f64 - 9.;
                        match column % 6 {
                            0 => 2.0_f64.powi(45) + v,
                            1 => v * 1e200,
                            2 => v * 1e-200,
                            3 => {
                                if row % 2 == 0 {
                                    0.0
                                } else {
                                    -0.0
                                }
                            }
                            4 => 1.0,
                            _ => v,
                        }
                    })
                })
                .collect(),
        )
        .unwrap();
        let mut summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
        for direct in [true, false] {
            summary.direct_centering = direct;
            let view = CenteredRows::new(&raw, &summary).unwrap();
            for selected in [
                (0..columns - 1).collect::<Vec<_>>(),
                (0..columns - 1).rev().step_by(2).collect(),
            ] {
                let scales: Vec<_> = selected
                    .iter()
                    .enumerate()
                    .map(|(i, _)| [1., 2., 0.5][i % 3])
                    .collect();
                for (first, rows) in [(0, 1), (1, 7), (7, 249), (256, 256), (512, 1)] {
                    let expected = scalar_panel(view, first, rows, &selected, &scales).unwrap();
                    let mut actual = vec![f64::NAN; expected.len()];
                    view.pack_panel(
                        first,
                        rows,
                        Some(&selected),
                        Some(&scales),
                        2.,
                        &mut actual,
                        &mut vec![0.; columns],
                    )
                    .unwrap();
                    assert_eq!(
                        actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                        expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}

#[test]
fn bulk_refusal_replays_scalar_error_and_validates_shape() {
    let raw = DenseMatrix::from_row_major(2, 2, vec![1., 2., 3., -2.]).unwrap();
    let summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    let view = CenteredRows::new(&raw, &summary).unwrap();
    let expected = scalar_panel(view, 0, 2, &[0], &[f64::from_bits(1)]).unwrap_err();
    let actual = view
        .pack_panel(
            0,
            2,
            Some(&[0]),
            Some(&[f64::from_bits(1)]),
            2.,
            &mut [0.; 4],
            &mut [0.; 2],
        )
        .unwrap_err();
    assert_eq!(actual, expected);
    for (first, rows, selected, scale) in
        [(0, 0, 0, 1.), (2, 1, 0, 1.), (0, 1, 1, 1.), (0, 1, 0, 0.)]
    {
        let error = view
            .pack_panel(
                first,
                rows,
                Some(&[selected]),
                Some(&[scale]),
                2.,
                &mut [0.; 4],
                &mut [0.; 2],
            )
            .unwrap_err();
        assert_eq!(error.code, crate::ErrorCode::InvalidArgument);
    }
}

#[test]
fn extreme_subtraction_keeps_scalar_scaled_fallback() {
    // Isolate the existing overflow-safe evaluator at its bulk boundary. The
    // deliberately unevaluated high/low center cancels in original units.
    let raw = DenseMatrix::from_row_major(1, 2, vec![f64::MAX * 0.75, f64::MAX * 0.75]).unwrap();
    let mut summary = CenteredSummary::build(&raw, &mut SystemFitReservation).unwrap();
    summary.centers_hi.fill(-f64::MAX * 0.75);
    summary.centers_lo.fill(f64::MAX * 0.75);
    summary.direct_centering = false;
    let view = CenteredRows::new(&raw, &summary).unwrap();
    let expected = scalar_panel(view, 0, 1, &[0], &[1.]).unwrap();
    let mut actual = [0.; 2];
    view.pack_panel(0, 1, Some(&[0]), Some(&[1.]), 2., &mut actual, &mut [0.; 2])
        .unwrap();
    assert_eq!(
        actual.map(f64::to_bits).as_slice(),
        expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
}
