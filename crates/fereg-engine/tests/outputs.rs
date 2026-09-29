use fereg_engine::{
    data::*,
    fe::{Design, Weights},
    outputs::{self, Kind, OlsFrame, Request},
    threads::ThreadTeam,
};
mod support;
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() < 2e-10 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
fn full_request(design: &Design<'_>) -> Request {
    Request {
        residuals: true,
        xb: true,
        xbd: true,
        absorbed: true,
        drop_reason: true,
        saved: design
            .layout()
            .effects
            .iter()
            .enumerate()
            .flat_map(|(e, l)| (0..l.channels).map(move |c| (e, c)))
            .collect(),
    }
}
fn compare(left: &outputs::Outputs, right: &outputs::Outputs) {
    assert_eq!(left.certificate, right.certificate);
    for (a, b) in left.columns.iter().zip(&right.columns) {
        assert_eq!(a.kind, b.kind);
        assert_eq!(
            a.values.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            b.values.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
    }
}
#[test]
fn requested_banks_scatter_once_and_preserve_disconnected_sloped_gauge() {
    let mut sample = support::prepared(&[6, 4, 6], true, 30, 0);
    let n = sample.n_active();
    let weights = (0..n).map(|i| 1. + (i % 3) as f64).collect::<Vec<_>>();
    let finite = DenseMatrix::from_columns(&[(0..n).map(|i| i as f64 / 100.).collect()]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let alpha = DenseMatrix::from_columns(&[(0..design.layout().total)
        .map(|i| i as f64 / 4. - 7.)
        .collect()])
    .unwrap();
    let fe = design.multiply(&alpha, &team).unwrap();
    let constant =
        (0..n).map(|i| weights[i] * fe[(i, 0)]).sum::<f64>() / weights.iter().sum::<f64>();
    sample.outcome = (0..n).map(|i| 2. * finite[(i, 0)] + fe[(i, 0)]).collect();
    sample.n_original = n + 4;
    sample.n_candidate = n + 3;
    sample.original_row_by_active = (0..n)
        .map(|i| i + 1 + if i >= n / 2 { 2 } else { 0 })
        .collect();
    sample.drop_reasons = vec![DropReason::MissingOutcome; n + 4];
    sample.drop_reasons[0] = DropReason::ExcludedByIfIn;
    for &row in sample.retained_rows().to_vec().iter() {
        sample.drop_reasons[row] = DropReason::Retained;
    }
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let residuals = vec![0.; n];
    let beta = [2.];
    let frame = OlsFrame {
        design: &design,
        finite_design: &finite,
        coefficients: &beta,
        absorbed_coefficients: &alpha,
        residuals: &residuals,
        reported_constant: constant,
    };
    let request = full_request(&design);
    let result = outputs::ols(&frame, &request, 1e-10, &team).unwrap();
    for workers in [2, 4, 8] {
        compare(
            &result,
            &outputs::ols(&frame, &request, 1e-10, &ThreadTeam::new(workers).unwrap()).unwrap(),
        );
    }
    for (i, &row) in sample.retained_rows().iter().enumerate() {
        close(
            result.get(Kind::Xb).unwrap()[row],
            2. * finite[(i, 0)] + constant,
        );
        close(result.get(Kind::Xbd).unwrap()[row], sample.outcome[i]);
        close(
            result.get(Kind::Absorbed).unwrap()[row],
            fe[(i, 0)] - constant,
        );
        let sum = request
            .saved
            .iter()
            .map(|&(effect, channel)| result.get(Kind::Saved { effect, channel }).unwrap()[row])
            .sum::<f64>();
        close(sum, result.get(Kind::Absorbed).unwrap()[row]);
        for &(effect, channel) in &request.saved {
            if channel == 0 {
                continue;
            }
            let level = design.topology().effect_partition(effect).unwrap().codes()[i] as usize;
            let coefficient = alpha[(design.coefficient_index(effect, channel, level).unwrap(), 0)];
            close(
                result.get(Kind::Saved { effect, channel }).unwrap()[row],
                coefficient * design.basis_value(effect, channel, i).unwrap(),
            );
        }
    }
    for effect in [1, 2] {
        for component in 0..2 {
            let centered = (0..n)
                .filter(|i| i % 2 == component)
                .map(|i| {
                    weights[i]
                        * result.get(Kind::Saved { effect, channel: 0 }).unwrap()
                            [sample.retained_rows()[i]]
                })
                .sum::<f64>();
            close(centered, 0.);
        }
    }
    for row in 0..sample.n_original() {
        if sample.is_retained(row) {
            continue;
        }
        for column in &result.columns {
            if column.kind != Kind::DropReason {
                assert!(column.values[row].is_nan());
            }
        }
        assert_eq!(
            result.get(Kind::DropReason).unwrap()[row],
            f64::from(sample.drop_reason(row).unwrap().code())
        );
    }
    assert_eq!(
        alpha.as_slice(),
        (0..design.layout().total)
            .map(|i| i as f64 / 4. - 7.)
            .collect::<Vec<_>>()
    );
}
#[test]
fn request_selection_never_changes_fit_or_creates_hidden_banks() {
    let mut sample = support::slope_sample(true, &[0, 0, 1, 1], &[vec![-1., 1., -1., 1.]]);
    sample.outcome = vec![2., 4., 3., 7.];
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let rhs = DenseMatrix::from_columns(&[sample.outcome.clone()]).unwrap();
    let fit = design.direct_candidate(&rhs, &team).unwrap();
    let finite = DenseMatrix::zeros(4, 0).unwrap();
    let frame = OlsFrame {
        design: &design,
        finite_design: &finite,
        coefficients: &[],
        absorbed_coefficients: &fit.coefficients,
        residuals: fit.residuals.as_slice(),
        reported_constant: 4.,
    };
    let full = outputs::ols(&frame, &full_request(&design), 1e-10, &team).unwrap();
    let none = outputs::ols(&frame, &Request::default(), 1e-10, &team).unwrap();
    assert!(none.columns.is_empty());
    assert!(none.certificate.is_none());
    for mask in 1..16 {
        let request = Request {
            residuals: mask & 1 != 0,
            xb: mask & 2 != 0,
            xbd: mask & 4 != 0,
            absorbed: mask & 8 != 0,
            ..Request::default()
        };
        let result = outputs::ols(&frame, &request, 1e-10, &team).unwrap();
        assert_eq!(result.columns.len(), (mask as u32).count_ones() as usize);
        for column in result.columns {
            assert_eq!(Some(column.values.as_slice()), full.get(column.kind));
        }
    }
    let selected = outputs::ols(
        &frame,
        &Request {
            saved: vec![(0, 1)],
            ..Request::default()
        },
        1e-10,
        &team,
    )
    .unwrap();
    assert_eq!(selected.columns.len(), 1);
    assert_eq!(
        selected.columns[0].values,
        full.get(Kind::Saved {
            effect: 0,
            channel: 1
        })
        .unwrap()
    );
}
#[test]
fn stale_frames_bad_channels_and_unsupported_constant_fail_before_return() {
    let mut sample = support::slope_sample(false, &[0, 0, 1, 1], &[vec![-1., 1., -1., 1.]]);
    sample.outcome = vec![-2., 2., -3., 3.];
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let rhs = DenseMatrix::from_columns(&[sample.outcome.clone()]).unwrap();
    let fit = design.direct_candidate(&rhs, &team).unwrap();
    let finite = DenseMatrix::zeros(4, 0).unwrap();
    let frame = OlsFrame {
        design: &design,
        finite_design: &finite,
        coefficients: &[],
        absorbed_coefficients: &fit.coefficients,
        residuals: fit.residuals.as_slice(),
        reported_constant: 0.,
    };
    let request = full_request(&design);
    outputs::ols(&frame, &request, 1e-10, &team).unwrap();
    for saved in [vec![(0, 0), (0, 0)], vec![(1, 0)], vec![(0, 1)]] {
        assert!(outputs::ols(
            &frame,
            &Request {
                saved,
                ..Request::default()
            },
            1e-10,
            &team
        )
        .is_err());
    }
    let bad = OlsFrame {
        residuals: &[1.; 4],
        ..frame
    };
    assert!(outputs::ols(&bad, &request, 1e-10, &team).is_err());
    let bad = OlsFrame {
        reported_constant: 1.,
        ..frame
    };
    assert!(outputs::ols(&bad, &request, 1e-10, &team).is_err());
    let wrong = DenseMatrix::zeros(fit.coefficients.nrows(), 1).unwrap();
    let bad = OlsFrame {
        absorbed_coefficients: &wrong,
        ..frame
    };
    assert!(outputs::ols(&bad, &request, 1e-10, &team).is_err());
}
#[test]
fn no_fe_model_returns_zero_absorbed_and_retains_ordinary_constant() {
    let mut sample = support::prepared(&[], false, 2, 0);
    sample.outcome.fill(3.);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let n = sample.n_active();
    let finite = DenseMatrix::from_columns(&[vec![1.; n]]).unwrap();
    let alpha = DenseMatrix::zeros(0, 1).unwrap();
    let frame = OlsFrame {
        design: &design,
        finite_design: &finite,
        coefficients: &[3.],
        absorbed_coefficients: &alpha,
        residuals: &vec![0.; n],
        reported_constant: 0.,
    };
    let output = outputs::ols(
        &frame,
        &full_request(&design),
        1e-10,
        &ThreadTeam::new(1).unwrap(),
    )
    .unwrap();
    assert_eq!(output.get(Kind::Absorbed).unwrap(), vec![0.; n]);
    assert_eq!(output.get(Kind::Xb).unwrap(), vec![3.; n]);
}
