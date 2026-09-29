use fereg_engine::{
    data::{PreparedWeights, ReferenceWeights, WeightKind},
    ppml::family::*,
    spec::Estimator,
    threads::ThreadTeam,
    ErrorCode,
};
fn close(a: f64, b: f64, t: f64) {
    assert!(
        (a - b).abs() <= t * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
#[test]
fn fractional_likelihood_deviance_and_working_state_match_analytic_values() {
    let y = [0., 1., 2., 0.5];
    let mu = [0.25_f64, 1.25, 2.5, 0.75];
    let eta = mu.map(f64::ln);
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: vec![4., 2., 3., 5.],
    };
    let q = ReferenceWeights::new(&raw, 4, Estimator::Ppml).unwrap();
    let offset = [-2., 1., 0., 3.];
    let family = Family::new(&y, q, Some(&offset)).unwrap();
    let team = ThreadTeam::new(2).unwrap();
    let actual = family.evaluate(&eta, &team).unwrap();
    let factorial = [0., 0., 2_f64.ln(), (std::f64::consts::PI.sqrt() / 2.).ln()];
    let mut deviance = 0.;
    let mut likelihood = 0.;
    for i in 0..4 {
        likelihood += q.value(i) * (y[i] * eta[i] - mu[i] - factorial[i]);
        deviance += 2.
            * q.value(i)
            * if y[i] == 0. {
                mu[i]
            } else {
                y[i] * (y[i] / mu[i]).ln() - y[i] + mu[i]
            };
    }
    close(actual.objective.deviance, deviance, 3e-14);
    close(actual.objective.log_likelihood, likelihood, 3e-14);
    let mut h = vec![0.; 4];
    let mut z = vec![0.; 4];
    family
        .working_into(&eta, &actual, &mut h, &mut z, &team)
        .unwrap();
    for i in 0..4 {
        close(h[i], q.value(i) * mu[i], 3e-14);
        close(z[i], eta[i] - offset[i] + (y[i] - mu[i]) / mu[i], 3e-14);
    }
    assert_eq!(z[0], eta[0] - offset[0] - 1.);
}
#[test]
fn objective_and_working_arithmetic_are_identical_across_chunk_boundaries_and_workers() {
    for n in [1023, 1024, 1025, 4099] {
        let y: Vec<_> = (0..n).map(|i| [0., 0.25, 1., 5., 13.][i % 5]).collect();
        let eta: Vec<_> = (0..n).map(|i| ((i % 17) as f64 - 8.) / 4.).collect();
        let raw = PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values: (0..n).map(|i| (1 + i % 7) as f64).collect(),
        };
        let q = ReferenceWeights::new(&raw, n, Estimator::Ppml).unwrap();
        let family = Family::new(&y, q, None).unwrap();
        let mut baseline = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let actual = family.evaluate(&eta, &team).unwrap();
            let mut h = vec![0.; n];
            let mut z = vec![0.; n];
            family
                .working_into(&eta, &actual, &mut h, &mut z, &team)
                .unwrap();
            let result = (actual, h, z);
            if let Some(expected) = &baseline {
                assert_eq!(&result, expected);
            } else {
                baseline = Some(result);
            }
        }
    }
}
#[test]
fn frequency_objective_matches_literal_row_replication() {
    let y = [0., 0.25, 2.];
    let eta = [-2., 0.3, 1.4];
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: vec![2., 3., 4.],
    };
    let q = ReferenceWeights::new(&raw, 3, Estimator::Ppml).unwrap();
    let expanded_y = [0., 0., 0.25, 0.25, 0.25, 2., 2., 2., 2.];
    let expanded_eta = [-2., -2., 0.3, 0.3, 0.3, 1.4, 1.4, 1.4, 1.4];
    let unit = ReferenceWeights::new(&PreparedWeights::None, 9, Estimator::Ppml).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let a = Family::new(&y, q, None)
        .unwrap()
        .evaluate(&eta, &team)
        .unwrap();
    let b = Family::new(&expanded_y, unit, None)
        .unwrap()
        .evaluate(&expanded_eta, &team)
        .unwrap();
    close(a.objective.deviance, b.objective.deviance, 1e-14);
    close(
        a.objective.log_likelihood,
        b.objective.log_likelihood,
        1e-14,
    );
}
#[test]
fn means_are_not_clipped_and_near_saturation_deviance_keeps_small_positive_values() {
    for eta in [-744., -700., 700.] {
        assert!(mean_from_eta(eta).unwrap() > 0.);
    }
    for eta in [-1000., 1000., f64::NAN, f64::INFINITY] {
        assert_eq!(
            mean_from_eta(eta).unwrap_err().code,
            ErrorCode::NonConvergence
        );
    }
    assert_eq!(deviance_contribution(0., 2.).unwrap(), 4.);
    assert_eq!(deviance_contribution(2., 2.).unwrap(), 0.);
    let y = 1_f64 + 1e-8;
    let delta = y - 1.;
    let expected = delta * delta - delta.powi(3) / 3. + delta.powi(4) / 6.;
    let actual = deviance_contribution(y, 1.).unwrap();
    assert!(actual > 0.);
    assert!((actual / expected - 1.).abs() < 1e-14);
}
#[test]
fn domains_and_stale_working_frames_fail_without_changing_the_accepted_evaluation() {
    let q = ReferenceWeights::new(&PreparedWeights::None, 2, Estimator::Ppml).unwrap();
    assert_eq!(
        Family::new(&[0., 0.], q, None).err().unwrap().code,
        ErrorCode::InsufficientObservations
    );
    assert_eq!(
        Family::new(&[-1., 1.], q, None).err().unwrap().code,
        ErrorCode::InvalidData
    );
    assert!(Family::new(&[1., 2.], q, Some(&[0.])).is_err());
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Analytic,
        values: vec![1.; 2],
    };
    let aw = ReferenceWeights::new(&raw, 2, Estimator::Ols).unwrap();
    assert_eq!(
        Family::new(&[1., 2.], aw, None).err().unwrap().code,
        ErrorCode::UnsupportedWeight
    );
    let family = Family::new(&[1., 2.], q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let accepted = family.evaluate(&[0., 0.], &team).unwrap();
    let before = accepted.objective;
    assert!(family.evaluate(&[0., 1000.], &team).is_err());
    assert!(family
        .working_into(&[0., 1.], &accepted, &mut [0.; 2], &mut [0.; 2], &team)
        .is_err());
    assert_eq!(accepted.objective, before);
    assert_eq!(accepted.mean, vec![1.; 2]);
}

#[test]
fn final_hessian_uses_exact_accepted_weights_without_a_fisher_step() {
    let n = 2051;
    let y: Vec<_> = (0..n).map(|i| (i % 7) as f64).collect();
    let offset: Vec<_> = (0..n).map(|i| (i % 5) as f64 * 0.2).collect();
    let eta: Vec<_> = offset
        .iter()
        .enumerate()
        .map(|(i, o)| o + (i % 11) as f64 * 0.1)
        .collect();
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: (0..n).map(|i| (1 + i % 3) as f64).collect(),
    };
    let q = ReferenceWeights::new(&raw, n, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, Some(&offset)).unwrap();
    let mut baseline = None;
    for workers in [1, 2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let accepted = family.evaluate(&eta, &team).unwrap();
        let mut h = vec![f64::NAN; n];
        let mut target = h.clone();
        family
            .hessian_into(&eta, &accepted, &mut h, &mut target, &team)
            .unwrap();
        for row in 0..n {
            assert_eq!(h[row], q.value(row) * eta[row].exp());
            assert_eq!(target[row], eta[row] - offset[row]);
        }
        if let Some(ref expected) = baseline {
            assert_eq!(&(h.clone(), target.clone()), expected);
        } else {
            baseline = Some((h.clone(), target.clone()));
        }
        let mut fisher_h = vec![0.; n];
        let mut fisher_z = vec![0.; n];
        family
            .working_into(&eta, &accepted, &mut fisher_h, &mut fisher_z, &team)
            .unwrap();
        assert_eq!(h, fisher_h);
        assert_ne!(target, fisher_z);
        let mut changed = eta.clone();
        changed[1024] += 0.25;
        assert_eq!(
            family
                .hessian_into(&changed, &accepted, &mut h, &mut target, &team)
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(accepted, family.evaluate(&eta, &team).unwrap());
    }
}
#[test]
fn final_hessian_does_not_evaluate_unneeded_overflowing_fisher_corrections() {
    let q = ReferenceWeights::new(&PreparedWeights::None, 2, Estimator::Ppml).unwrap();
    let family = Family::new(&[0., 1.], q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let eta = [-744., -744.];
    let accepted = family.evaluate(&eta, &team).unwrap();
    let mut h = [0.; 2];
    let mut target = [0.; 2];
    family
        .hessian_into(&eta, &accepted, &mut h, &mut target, &team)
        .unwrap();
    assert_eq!(target, eta);
    assert!(h.iter().all(|w| *w > 0.));
    assert_eq!(
        family
            .working_into(&eta, &accepted, &mut h, &mut target, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    assert!(family
        .hessian_into(&eta, &accepted, &mut h[..1], &mut target, &team)
        .is_err());
}
