use fereg_engine::{data::*, ppml::family::Family, spec::Estimator, threads::ThreadTeam};
mod support;
fn close(a: f64, b: f64, t: f64) {
    assert!(
        (a - b).abs() < t * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
#[test]
fn global_start_compensates_offsets_while_the_null_model_ignores_them() {
    let y = [0., 1., 2., 4.];
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: vec![1., 3., 2., 4.],
    };
    let q = ReferenceWeights::new(&raw, 4, Estimator::Ppml).unwrap();
    let baseline = Family::new(&y, q, None).unwrap().null_objective().unwrap();
    close(baseline.mean, 2.3, 1e-15);
    let factorial = [0., 0., 2_f64.ln(), 24_f64.ln()];
    let ll: f64 = (0..4)
        .map(|i| q.value(i) * (y[i] * 2.3_f64.ln() - 2.3 - factorial[i]))
        .sum();
    close(baseline.log_likelihood, ll, 3e-14);
    let team = ThreadTeam::new(1).unwrap();
    for offset in [-700., 0., 700.] {
        let values = [offset; 4];
        let family = Family::new(&y, q, Some(&values)).unwrap();
        let start = family.global_start().unwrap();
        close(start, 2.3_f64.ln() - offset, 1e-14);
        let evaluation = family.evaluate(&[start + offset; 4], &team).unwrap();
        for &mean in &evaluation.mean {
            close(mean, 2.3, 1e-13);
        }
        assert_eq!(family.null_objective().unwrap(), baseline);
    }
}
#[test]
fn one_categorical_effect_has_an_analytic_proposal_and_zero_groups_decline() {
    let sample = support::prepared(&[2], false, 3, 0);
    let y = [0., 1., 2., 4., 6., 3.];
    let offset = [0., 0.3, 0.6, -0.2, -0.4, 0.5];
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: vec![1., 2., 3., 1., 2., 3.],
    };
    let q = ReferenceWeights::new(&raw, 6, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, Some(&offset)).unwrap();
    let topology = sample.topology.as_ref().unwrap();
    let global = family.global_start().unwrap();
    let mut baseline = None;
    for workers in [1, 2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let proposed = family.one_fe_start(topology, &team).unwrap().unwrap();
        for (level, &coefficient) in proposed.iter().enumerate() {
            let rows = topology
                .effect_partition(0)
                .unwrap()
                .membership()
                .members(level)
                .unwrap();
            let numerator: f64 = rows.iter().map(|&i| q.value(i) * y[i]).sum();
            let denominator: f64 = rows.iter().map(|&i| q.value(i) * offset[i].exp()).sum();
            close(coefficient, (numerator / denominator).ln(), 1e-14);
        }
        let eta: Vec<_> = (0..6).map(|i| offset[i] + proposed[i % 2]).collect();
        let global_eta: Vec<_> = offset.iter().map(|v| v + global).collect();
        assert!(
            family.evaluate(&eta, &team).unwrap().objective.deviance
                <= family
                    .evaluate(&global_eta, &team)
                    .unwrap()
                    .objective
                    .deviance
        );
        if let Some(expected) = &baseline {
            assert_eq!(&proposed, expected);
        } else {
            baseline = Some(proposed);
        }
    }
    let y = [0., 1., 0., 2., 0., 3.];
    let family = Family::new(&y, q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let global = family.global_start().unwrap();
    let accepted = family.evaluate(&[global; 6], &team).unwrap();
    assert!(family.one_fe_start(topology, &team).unwrap().is_none());
    assert_eq!(family.evaluate(&[global; 6], &team).unwrap(), accepted);
}
#[test]
fn multi_effect_and_sloped_topologies_do_not_add_other_start_algorithms() {
    let y = [1.; 6];
    let q = ReferenceWeights::new(&PreparedWeights::None, 6, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    for sample in [
        support::prepared(&[3, 2], false, 2, 0),
        support::prepared(&[3], true, 2, 0),
    ] {
        assert!(family
            .one_fe_start(sample.topology.as_ref().unwrap(), &team)
            .unwrap()
            .is_none());
    }
    let wrong = support::prepared(&[3], false, 3, 0);
    assert!(family
        .one_fe_start(wrong.topology.as_ref().unwrap(), &team)
        .is_err());
}
#[test]
fn null_mean_log_fallback_retains_rescuable_subnormal_weighted_outcomes() {
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: vec![1., f64::from_bits(1)],
    };
    let q = ReferenceWeights::new(&raw, 2, Estimator::Ppml).unwrap();
    let family = Family::new(&[0., 1e300], q, None).unwrap();
    let actual = family.null_objective().unwrap();
    let expected = f64::from_bits(1) * 1e300;
    assert!(actual.mean > 0.);
    assert!((actual.mean / expected - 1.).abs() < 2e-13);
    assert!(actual.log_likelihood.is_finite());
    assert!((family.global_start().unwrap().exp() / expected - 1.).abs() < 2e-13);
}
