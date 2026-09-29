#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::{PreparedWeights, ReferenceWeights, WeightKind},
    ppml::family::Family,
    spec::Estimator,
    threads::ThreadTeam,
};
use fereg_oracle::explicit_ppml::{fit_explicit_ppml, PpmlProblem, PpmlWeights};
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() < 2e-9 * a.abs().max(b.abs()).max(1.),
        "seed {seed}: {a:.17e} != {b:.17e}"
    );
}
fn log_factorial(y: f64) -> f64 {
    if y.fract() == 0. {
        (1..=y as usize).map(|i| (i as f64).ln()).sum()
    } else {
        0.5 * std::f64::consts::PI.ln()
            + (0..=y as usize).map(|i| (i as f64 + 0.5).ln()).sum::<f64>()
    }
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let n = 31 + seed % 41;
        let y: Vec<_> = (0..n)
            .map(|i| ((i * 13 + seed * 7) % 17) as f64 / 2.)
            .collect();
        let x: Vec<_> = (0..n)
            .map(|i| vec![1., ((i * 11 + seed) % 19) as f64 / 9. - 1.])
            .collect();
        let offset: Vec<_> = (0..n)
            .map(|i| ((i + seed) % 5) as f64 * 0.2 - 0.4)
            .collect();
        let weights: Vec<_> = (0..n).map(|i| (1 + (i + seed) % 4) as f64).collect();
        let (raw, oracle_weights) = match seed % 3 {
            0 => (PreparedWeights::None, PpmlWeights::Unit),
            1 => (
                PreparedWeights::Provided {
                    kind: WeightKind::Frequency,
                    values: weights.clone(),
                },
                PpmlWeights::Frequency(weights.iter().map(|q| *q as u32).collect()),
            ),
            _ => (
                PreparedWeights::Provided {
                    kind: WeightKind::Probability,
                    values: weights.clone(),
                },
                PpmlWeights::Probability(weights),
            ),
        };
        let q = ReferenceWeights::new(&raw, n, Estimator::Ppml).unwrap();
        let problem = PpmlProblem {
            design: x.clone(),
            outcome: y.clone(),
            offset: offset.clone(),
            weights: oracle_weights,
            clusters: vec![],
        };
        let expected = fit_explicit_ppml(&problem).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let family = Family::new(&y, q, Some(&offset)).unwrap();
        let sum_q: f64 = (0..n).map(|i| q.value(i)).sum();
        let sum_qy: f64 = (0..n).map(|i| q.value(i) * y[i]).sum();
        let sum_q_offset: f64 = (0..n).map(|i| q.value(i) * offset[i].exp()).sum();
        close(
            family.global_start().unwrap(),
            (sum_qy / sum_q_offset).ln(),
            seed,
        );
        let null = family.null_objective().unwrap();
        close(null.mean, sum_qy / sum_q, seed);
        let null_ll: f64 = (0..n)
            .map(|i| {
                q.value(i) * (y[i] * (sum_qy / sum_q).ln() - sum_qy / sum_q - log_factorial(y[i]))
            })
            .sum();
        close(null.log_likelihood, null_ll, seed);
        let actual = family.evaluate(&expected.eta, &one).unwrap();
        let mut h = vec![0.; n];
        let mut z = vec![0.; n];
        family
            .working_into(&expected.eta, &actual, &mut h, &mut z, &one)
            .unwrap();
        close(actual.objective.deviance, expected.deviance, seed);
        let factorial: f64 = (0..n).map(|i| q.value(i) * log_factorial(y[i])).sum();
        close(
            actual.objective.log_likelihood,
            expected.log_likelihood_kernel - factorial,
            seed,
        );
        let q_values: Vec<_> = (0..n).map(|i| q.value(i)).collect();
        let legacy = fereg_core::poisson_objective(&y, &actual.mean, &q_values).unwrap();
        close(actual.objective.deviance, legacy.deviance, seed);
        close(actual.objective.log_likelihood, legacy.log_likelihood, seed);
        for left in 0..2 {
            for right in 0..2 {
                close(
                    (0..n).map(|i| h[i] * x[i][left] * x[i][right]).sum(),
                    expected.hessian[left][right],
                    seed,
                );
            }
        }
        for i in 0..n {
            close(h[i], q.value(i) * expected.mean[i], seed);
            close(
                z[i],
                expected.eta[i] - offset[i] + (y[i] - expected.mean[i]) / expected.mean[i],
                seed,
            );
        }
        if seed % 20 == 0 {
            assert_eq!(actual, family.evaluate(&expected.eta, &eight).unwrap());
            let mut other_h = vec![0.; n];
            let mut other_z = vec![0.; n];
            family
                .working_into(&expected.eta, &actual, &mut other_h, &mut other_z, &eight)
                .unwrap();
            assert_eq!(h, other_h);
            assert_eq!(z, other_z);
        }
    }
}
#[test]
fn poisson_family_matches_independent_newton_states_and_pinned_legacy() {
    campaign(0, 200);
}
#[test]
#[ignore = "long Poisson family differential campaign"]
fn two_thousand_poisson_family_states() {
    campaign(200, 2000);
}
