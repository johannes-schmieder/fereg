use fereg_engine::{
    data::{DenseMatrix, PreparedWeights, ReferenceWeights, WeightKind},
    dof::ResidualDof,
    factor::CategoricalCodeView,
    spec::Estimator,
    statistics,
    threads::ThreadTeam,
    vce::{self, Kind, Problem},
    wls,
};
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() < 2e-10 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
#[test]
fn reference_weights_preserve_frequency_and_normalize_scale_without_overflow() {
    let unit = PreparedWeights::None;
    let w = ReferenceWeights::new(&unit, 4, Estimator::Ols).unwrap();
    assert_eq!(w.effective(), 4);
    assert_eq!(w.kind(), None);
    assert_eq!(w.value(3), 1.);
    for scale in [1e-300, 1., 1e300] {
        let raw = PreparedWeights::Provided {
            kind: WeightKind::Analytic,
            values: vec![scale, 2. * scale, 4. * scale, 8. * scale],
        };
        let w = ReferenceWeights::new(&raw, 4, Estimator::Ols).unwrap();
        for i in 0..4 {
            close(w.value(i), (1usize << i) as f64 * 4. / 15.);
        }
        close(w.log_raw_sum.unwrap(), scale.ln() + 15_f64.ln());
        assert!(ReferenceWeights::new(&raw, 4, Estimator::Ppml).is_err());
    }
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: vec![1e308; 4],
    };
    let w = ReferenceWeights::new(&raw, 4, Estimator::Ppml).unwrap();
    assert_eq!(w.raw_sum, None);
    assert_eq!(w.value(0), 1.);
    let fw = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: vec![1., 2., 4., 8.],
    };
    let w = ReferenceWeights::new(&fw, 4, Estimator::Ols).unwrap();
    assert_eq!(w.effective(), 15);
    assert_eq!(w.value(3), 8.);
    close(w.robust_root(3).powi(2), 8.);
    for value in [0., 1.5, u32::MAX as f64 + 1., f64::NAN] {
        let fw = PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values: vec![value],
        };
        assert!(ReferenceWeights::new(&fw, 1, Estimator::Ols).is_err());
    }
    assert!(ReferenceWeights::new(&unit, (1usize << 53) + 1, Estimator::Ols).is_err());
}
#[test]
fn dof_distinguishes_nested_fit_test_and_correction_policies() {
    let ols = ResidualDof::new(Estimator::Ols, 100, 4, 10, 20, &[7, 9]).unwrap();
    assert_eq!(
        (ols.structural, ols.reported, ols.fit, ols.correction),
        (86, 6, 66, 85)
    );
    close(ols.cluster_multiplier().unwrap(), 99. / 85. * 7. / 6.);
    let ppml = ResidualDof::new(Estimator::Ppml, 100, 4, 10, 20, &[7, 9]).unwrap();
    assert_eq!(
        (ppml.structural, ppml.reported, ppml.fit, ppml.correction),
        (86, 6, 86, 86)
    );
    close(ppml.robust_multiplier().unwrap(), 100. / 99.);
    close(ppml.cluster_multiplier().unwrap(), 7. / 6.);
    for (n, k, a, nested, clusters) in [
        (24, 4, 20, 0, vec![]),
        (24, 4, 1, 19, vec![3]),
        (24, 4, 1, 0, vec![1]),
    ] {
        assert!(ResidualDof::new(Estimator::Ols, n, k, a, nested, &clusters).is_err());
    }
}
#[test]
fn statistics_keep_missing_and_cluster_fit_dof() {
    let raw = PreparedWeights::None;
    let w = ReferenceWeights::new(&raw, 8, Estimator::Ols).unwrap();
    let y = [1., 2., 3., 4., 5., 6., 7., 8.];
    let within = [-3.5, -2.5, -1.5, -0.5, 0.5, 1.5, 2.5, 3.5];
    let r = [1., -1., 1., -1., 1., -1., 1., -1.];
    let dof = ResidualDof::new(Estimator::Ols, 8, 1, 1, 2, &[4]).unwrap();
    let s = statistics::ols(&y, &within, &r, w, true, dof).unwrap();
    close(s.rss, 8.);
    close(s.tss, 42.);
    close(s.within_tss, 42.);
    close(s.mss, 34.);
    close(s.rmse, 2_f64.sqrt());
    close(s.adjusted_r_squared.unwrap(), 1. - 2. / 6.);
    close(s.adjusted_within_r_squared.unwrap(), 1. - 2. / (42. / 5.));
    close(
        s.log_likelihood.unwrap(),
        -4. * (1. + (2. * std::f64::consts::PI).ln()),
    );
    let zeros = [0.; 8];
    let s = statistics::ols(&zeros, &zeros, &zeros, w, true, dof).unwrap();
    assert_eq!(s.r_squared, None);
    assert_eq!(s.within_r_squared, None);
    assert_eq!(s.log_likelihood, None);
    assert_eq!(s.null_log_likelihood, None);
}
#[test]
fn ppml_scalar_inference_uses_its_own_corrections_and_frequency_scores() {
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: vec![1., 2., 3., 4.],
    };
    let weights = ReferenceWeights::new(&raw, 4, Estimator::Ppml).unwrap();
    let x = DenseMatrix::from_row_major(4, 1, vec![1., 2., 3., 4.]).unwrap();
    let b = DenseMatrix::from_row_major(1, 1, vec![0.01]).unwrap();
    let residual = [1., -1., 2., -2.];
    let c = [0, 0, 1, 1];
    let clusters = [CategoricalCodeView::new(&c, 2)];
    let team = ThreadTeam::new(2).unwrap();
    let p = Problem {
        design: &x,
        bread: &b,
        residual: &residual,
        weights,
        clusters: &[],
        dof: ResidualDof::new(Estimator::Ppml, 10, 1, 0, 0, &[]).unwrap(),
    };
    let result = vce::estimate(&p, Kind::Robust, 5e-10, &team).unwrap();
    close(
        result.values[(0, 0)],
        (1. + 8. + 108. + 256.) * 0.0001 * 10. / 9.,
    );
    assert!(vce::estimate(&p, Kind::Iid, 5e-10, &team).is_err());
    let p = Problem {
        clusters: &clusters,
        dof: ResidualDof::new(Estimator::Ppml, 10, 1, 0, 0, &[2]).unwrap(),
        ..p
    };
    let result = vce::estimate(&p, Kind::Cluster, 5e-10, &team).unwrap();
    close(result.values[(0, 0)], (9. + 196.) * 0.0001 * 2.);
}
#[test]
fn covariance_and_statistics_match_literal_frequency_replication() {
    let team = ThreadTeam::new(1).unwrap();
    let mut baseline: Option<Vec<f64>> = None;
    for expanded in [false, true] {
        let mut x = vec![];
        let mut y = vec![];
        let mut clusters = vec![];
        let mut frequencies = vec![];
        for i in 0..40 {
            for _ in 0..if expanded { 1 + i % 4 } else { 1 } {
                x.push(1.);
                x.push(i as f64 / 40.);
                y.push(2. + i as f64 / 40. + ((i * 7 % 13) as f64 - 6.) / 4.);
                clusters.push((i % 7) as u32);
                frequencies.push((1 + i % 4) as f64);
            }
        }
        let raw = if expanded {
            PreparedWeights::None
        } else {
            PreparedWeights::Provided {
                kind: WeightKind::Frequency,
                values: frequencies,
            }
        };
        let weights = ReferenceWeights::new(&raw, y.len(), Estimator::Ols).unwrap();
        let x = DenseMatrix::from_row_major(y.len(), 2, x).unwrap();
        let w = (0..y.len()).map(|i| weights.value(i)).collect::<Vec<_>>();
        let slots = (0..2)
            .map(|j| wls::Slot {
                role: wls::Role::Ordinary,
                reference_norm: (0..y.len()).fold(0_f64, |n, i| n.hypot(w[i].sqrt() * x[(i, j)])),
            })
            .collect::<Vec<_>>();
        let fit = wls::solve(
            &wls::Problem {
                design: &x,
                outcome: &y,
                weights: &w,
                slots: &slots,
                has_absorbed_intercept: false,
            },
            wls::Policy::default(),
            &team,
        )
        .unwrap();
        let mut values = vec![];
        for kind in [Kind::Iid, Kind::Robust, Kind::Cluster] {
            let c = [CategoricalCodeView::new(&clusters, 7)];
            let counts = if kind == Kind::Cluster {
                vec![7]
            } else {
                vec![]
            };
            let dof =
                ResidualDof::new(Estimator::Ols, weights.effective(), 1, 1, 0, &counts).unwrap();
            let p = Problem {
                design: &x,
                bread: &fit.bread,
                residual: &fit.residuals,
                weights,
                clusters: if kind == Kind::Cluster { &c } else { &[] },
                dof,
            };
            let result = vce::estimate(&p, kind, 5e-10, &team).unwrap();
            for workers in [2, 4, 8] {
                assert_eq!(
                    result,
                    vce::estimate(&p, kind, 5e-10, &ThreadTeam::new(workers).unwrap()).unwrap()
                );
            }
            values.extend_from_slice(result.values.as_slice());
            let stats = statistics::ols(&y, &y, &fit.residuals, weights, true, dof).unwrap();
            values.extend([stats.rss, stats.rmse, stats.log_likelihood.unwrap()]);
        }
        if let Some(expected) = &baseline {
            for (a, b) in values.iter().zip(expected) {
                close(*a, *b);
            }
        } else {
            baseline = Some(values);
        }
    }
}
#[test]
fn wald_preserves_undefined_rank_and_distribution() {
    let v = DenseMatrix::from_row_major(3, 3, vec![4., 0., 0., 0., 2., 0., 0., 0., 0.]).unwrap();
    for estimator in [Estimator::Ols, Estimator::Ppml] {
        let w = vce::wald(&[2., 2., 0.], &v, &[0, 1], estimator, 5e-10).unwrap();
        close(
            w.statistic.unwrap(),
            if estimator == Estimator::Ols { 1.5 } else { 3. },
        );
        assert_eq!(
            vce::wald(&[2., 2., 0.], &v, &[0, 1, 2], estimator, 5e-10)
                .unwrap()
                .statistic,
            None
        );
        assert_eq!(
            vce::wald(&[2., 2., 0.], &v, &[], estimator, 5e-10)
                .unwrap()
                .statistic,
            None
        );
        assert!(vce::wald(&[2., 2., 0.], &v, &[1, 0], estimator, 5e-10).is_err());
    }
}

#[test]
fn normalized_probability_weights_preserve_rescued_subnormals_and_failure_kind() {
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: vec![2., f64::from_bits(1)],
    };
    let w = ReferenceWeights::new(&raw, 2, Estimator::Ppml).unwrap();
    assert_eq!(w.value(0), 2.);
    assert_eq!(w.value(1), f64::from_bits(1));
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: vec![1e308, f64::from_bits(1)],
    };
    assert_eq!(
        ReferenceWeights::new(&raw, 2, Estimator::Ppml)
            .unwrap_err()
            .code,
        fereg_engine::ErrorCode::NonConvergence
    );
}

#[test]
fn cluster_tuple_limits_duplicates_and_zero_reported_rank() {
    let raw = PreparedWeights::None;
    let weights = ReferenceWeights::new(&raw, 16, Estimator::Ols).unwrap();
    let x = DenseMatrix::from_row_major(16, 1, vec![1.; 16]).unwrap();
    let b = DenseMatrix::from_row_major(1, 1, vec![1. / 16.]).unwrap();
    let residual = (0..16).map(|i| (i % 3) as f64 - 1.).collect::<Vec<_>>();
    let c = (0..16).map(|i| (i % 4) as u32 * 10).collect::<Vec<_>>();
    let views = vec![CategoricalCodeView::new(&c, 31); 10];
    let team = ThreadTeam::new(4).unwrap();
    let dof = ResidualDof::new(Estimator::Ols, 16, 1, 0, 0, &[4; 10]).unwrap();
    let p = Problem {
        design: &x,
        bread: &b,
        residual: &residual,
        weights,
        clusters: &views,
        dof,
    };
    let ten = vce::estimate(&p, Kind::Cluster, 5e-10, &team).unwrap();
    let p = Problem {
        clusters: &views[..1],
        dof: ResidualDof::new(Estimator::Ols, 16, 1, 0, 0, &[4]).unwrap(),
        ..p
    };
    let one = vce::estimate(&p, Kind::Cluster, 5e-10, &team).unwrap();
    close(ten.values[(0, 0)], one.values[(0, 0)]);
    assert_eq!(ten.subset_counts, vec![4; 1023]);
    let empty = DenseMatrix::zeros(16, 0).unwrap();
    let bread = DenseMatrix::zeros(0, 0).unwrap();
    let p = Problem {
        design: &empty,
        bread: &bread,
        ..p
    };
    let zero = vce::estimate(&p, Kind::Cluster, 5e-10, &team).unwrap();
    assert_eq!(zero.values.ncols(), 0);
    assert_eq!(zero.cluster_counts, vec![4]);
    let ppml = ReferenceWeights::new(&raw, 16, Estimator::Ppml).unwrap();
    let p = Problem {
        design: &x,
        bread: &b,
        weights: ppml,
        clusters: &views[..6],
        dof: ResidualDof::new(Estimator::Ppml, 16, 1, 0, 0, &[4]).unwrap(),
        ..p
    };
    assert!(vce::estimate(&p, Kind::Cluster, 5e-10, &team).is_err());
}
