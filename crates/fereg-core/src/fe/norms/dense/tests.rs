use super::super::reference;
use super::*;

#[test]
fn norm_handoff_dual_states_match_retained_independent_recurrences() {
    for workers in [1, 4, 8] {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for rows in [0, 1, 511, 512, 513, 1537] {
            for width in [1, 6, 17, 65] {
                for weighted in [false, true] {
                    let root = |r: usize| {
                        if weighted {
                            [1.0, 0.5, 1e-120][r % 3]
                        } else {
                            1.0
                        }
                    };
                    let matrix = DenseMatrix::from_row_major(
                        rows,
                        width,
                        (0..rows * width)
                            .map(|i| match i % 11 {
                                0 => -0.0,
                                1 => f64::from_bits(1),
                                2 => -1e150,
                                3 => 1e-300,
                                _ => ((i % 131) as f64 - 65.0) / 67.0,
                            })
                            .collect(),
                    )
                    .unwrap();
                    let mut output = vec![0.0; width];
                    let mut scales = output.clone();
                    let mut sums = output.clone();
                    let mut dense = output.clone();
                    let valid = team
                        .install(|| {
                            column_norms(
                                &matrix,
                                |r| Ok(root(r)),
                                &mut output,
                                &mut scales,
                                &mut sums,
                                &mut dense,
                                "value",
                                "norm",
                            )
                        })
                        .unwrap();
                    assert!(valid);
                    for c in 0..width {
                        let values = (0..rows)
                            .map(|r| root(r) * matrix[(r, c)])
                            .collect::<Vec<_>>();
                        let fixed = reference::fixed_leaf_norm_state(&values);
                        let whole = reference::legacy_scalar_norm_state(values.into_iter());
                        assert_eq!(
                            [output[c], scales[c], sums[c]].map(f64::to_bits),
                            [fixed.0, fixed.1, fixed.2].map(f64::to_bits)
                        );
                        assert_eq!(dense[c].to_bits(), whole.0.to_bits());
                    }
                }
            }
        }
    }
}

#[test]
fn norm_handoff_dual_state_preserves_certificate_error_precedence() {
    for workers in [1, 4, 8] {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for case in 0..4 {
            let mut values = vec![1.0; 513 * 65];
            if case == 0 {
                values[64] = f64::NAN;
            }
            if case == 1 {
                values.fill(f64::MAX);
            }
            if case == 2 {
                values[512 * 65] = f64::INFINITY;
            }
            let matrix = DenseMatrix::from_row_major(513, 65, values).unwrap();
            let root = |r| {
                if case != 1 && r == 512 {
                    Err(FeregError::new(ErrorCode::InvalidData, "root"))
                } else {
                    Ok(1.0)
                }
            };
            let mut output = vec![0.0; 65];
            let mut scales = output.clone();
            let mut sums = output.clone();
            let mut dense = output.clone();
            let expected = team.install(|| {
                crate::fe::norms::column_norms(
                    &matrix,
                    root,
                    &mut output,
                    &mut scales,
                    &mut sums,
                    &mut vec![0.0; 16384],
                    "value",
                    "norm",
                )
            });
            let actual = team.install(|| {
                column_norms(
                    &matrix,
                    root,
                    &mut output,
                    &mut scales,
                    &mut sums,
                    &mut dense,
                    "value",
                    "norm",
                )
            });
            assert_eq!(actual.unwrap_err(), expected.unwrap_err());
        }
    }
}

#[test]
fn norm_handoff_dense_only_overflow_declines_without_rejecting_fe_norm() {
    // Retained scalar and fixed-leaf witnesses straddle the finite boundary.
    let factor = f64::from_bits(9_194_085_752_445_048_521);
    let values = (0..1537)
        .map(|r| ((r % 173 + 1) as f64 / 91.0) * factor)
        .collect::<Vec<_>>();
    let whole = reference::legacy_scalar_norm_state(values.iter().copied());
    let fixed = reference::fixed_leaf_norm_state(&values);
    assert!(whole.0.is_infinite());
    assert!(fixed.0.is_finite());
    let matrix = DenseMatrix::from_row_major(values.len(), 1, values).unwrap();
    for workers in [1, 4, 8] {
        let team = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        let mut out = [0.0];
        let mut scales = [0.0];
        let mut sums = [0.0];
        let mut dense = [0.0];
        let valid = team
            .install(|| {
                column_norms(
                    &matrix,
                    |_| Ok(1.0),
                    &mut out,
                    &mut scales,
                    &mut sums,
                    &mut dense,
                    "value",
                    "norm",
                )
            })
            .unwrap();
        assert!(!valid);
        assert_eq!(out[0].to_bits(), fixed.0.to_bits());
    }
}
