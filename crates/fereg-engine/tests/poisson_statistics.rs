use fereg_engine::{
    data::*,
    ppml::irls,
    spec::{Estimator, Options},
    statistics,
    threads::ThreadTeam,
    wls::Role,
};
mod support;
fn log_factorial(y: f64) -> f64 {
    if y.fract() == 0. {
        (1..=y as usize).map(|i| (i as f64).ln()).sum()
    } else {
        0.5 * std::f64::consts::PI.ln()
            + (0..=y as usize).map(|i| (i as f64 + 0.5).ln()).sum::<f64>()
    }
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "{a:.17e} != {b:.17e}"
    );
}
#[test]
fn accepted_statistics_match_independent_half_integer_likelihoods_and_ignore_offset_in_the_null() {
    for levels in [&[][..], &[3, 4][..]] {
        for kind in [
            None,
            Some(WeightKind::Frequency),
            Some(WeightKind::Probability),
        ] {
            let mut sample = support::prepared(levels, false, 30, 0);
            let n = sample.n_active();
            sample.outcome = (0..n).map(|i| ((i * 7) % 13) as f64 * 0.5).collect();
            sample.weights = kind.map_or(PreparedWeights::None, |kind| PreparedWeights::Provided {
                kind,
                values: (0..n).map(|i| (1 + i % 5) as f64).collect(),
            });
            sample.adjustment =
                PreparedAdjustment::Offset((0..n).map(|i| (i % 7) as f64 * 0.08 - 0.24).collect());
            let raw = DenseMatrix::from_columns(&[vec![1.; n]]).unwrap();
            let roles = if levels.is_empty() {
                [Role::Ordinary]
            } else {
                [Role::AbsorbedIntercept]
            };
            let q = ReferenceWeights::new(&sample.weights, n, Estimator::Ppml).unwrap();
            let sum_q = (0..n).map(|i| q.value(i)).sum::<f64>();
            let sum_y = (0..n).map(|i| q.value(i) * sample.outcome[i]).sum::<f64>();
            let null_mean = sum_y / sum_q;
            let expected_null = (0..n)
                .map(|i| {
                    q.value(i)
                        * (sample.outcome[i] * null_mean.ln()
                            - null_mean
                            - log_factorial(sample.outcome[i]))
                })
                .sum::<f64>();
            let mut baseline = None;
            for workers in [1, 2, 4, 8] {
                let team = ThreadTeam::new(workers).unwrap();
                let fit = irls::fit_final_sample(
                    &irls::Problem {
                        sample: &sample,
                        raw: &raw,
                        roles: &roles,
                    },
                    Options::for_estimator(Estimator::Ppml),
                    &team,
                )
                .unwrap();
                let actual = statistics::ppml(&fit).unwrap();
                let expected = (0..n)
                    .map(|i| {
                        q.value(i)
                            * (sample.outcome[i] * fit.point.eta[i]
                                - fit.evaluation.mean[i]
                                - log_factorial(sample.outcome[i]))
                    })
                    .sum::<f64>();
                close(actual.log_likelihood, expected);
                close(actual.null_mean, null_mean);
                close(actual.null_log_likelihood, expected_null);
                close(actual.pseudo_r_squared, 1. - expected / expected_null);
                close(actual.weighted_outcome_sum, sum_y);
                assert_eq!(actual.deviance, fit.evaluation.objective.deviance);
                assert_eq!(actual.outer_iterations, fit.diagnostics.accepted_iterations);
                assert_eq!(
                    actual.inner_iterations,
                    fit.diagnostics.work.counts().summed_iterations
                );
                assert_eq!(actual.step_halvings, fit.diagnostics.step_halvings);
                assert!(actual.converged);
                if let Some(previous) = baseline {
                    assert_eq!(actual, previous);
                } else {
                    baseline = Some(actual);
                }
            }
        }
    }
}
#[test]
fn constant_offsets_of_either_sign_leave_the_reference_null_and_accepted_statistics_unchanged() {
    let mut baseline = None;
    for offset in [0_f64, -700., 700.] {
        let mut sample = support::prepared(&[], false, 20, 0);
        let n = sample.n_active();
        sample.outcome = (0..n).map(|i| ((i * 7) % 13) as f64 * 0.5).collect();
        sample.adjustment = PreparedAdjustment::Exposure(vec![offset.exp(); n]);
        let raw = DenseMatrix::from_columns(&[vec![1.; n]]).unwrap();
        let team = ThreadTeam::new(1).unwrap();
        let fit = irls::fit_final_sample(
            &irls::Problem {
                sample: &sample,
                raw: &raw,
                roles: &[Role::Ordinary],
            },
            Options::for_estimator(Estimator::Ppml),
            &team,
        )
        .unwrap();
        let stats = statistics::ppml(&fit).unwrap();
        close(fit.point.finite[0] + offset, stats.null_mean.ln());
        close(stats.log_likelihood, stats.null_log_likelihood);
        close(stats.pseudo_r_squared, 0.);
        if let Some((ll, mu0, ll0, sum_y)) = baseline {
            close(stats.log_likelihood, ll);
            assert_eq!(stats.null_mean, mu0);
            assert_eq!(stats.null_log_likelihood, ll0);
            assert_eq!(stats.weighted_outcome_sum, sum_y);
        } else {
            baseline = Some((
                stats.log_likelihood,
                stats.null_mean,
                stats.null_log_likelihood,
                stats.weighted_outcome_sum,
            ));
        }
    }
}
