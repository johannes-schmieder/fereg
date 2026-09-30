//! Independent observation products and Stirling likelihoods, plus pinned differentials.
use fereg_engine::{
    data::{DropReason, Prepared},
    outputs::{self, Kind, Outputs, PpmlRequest, Request},
    ppml::{irls, residuals::ResidualKind},
    statistics,
    threads::ThreadTeam,
    wls::Role,
};
use fereg_oracle::explicit_ppml::{PpmlProblem, PpmlResult, PpmlWeights};

fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "output seed={seed}: {a:.17e} != {b:.17e}"
    );
}
// NIST DLMF 5.11.1 and 5.11(ii): positive-real Stirling remainders are bounded
// by the first omitted term. Recurrence to z+32 makes that bound <4e-20 here.
// This does not use the production Lanczos coefficients or log-gamma routine.
// https://dlmf.nist.gov/5.11#E1
fn log_factorial(y: f64) -> f64 {
    let z = y + 1.;
    let t = z + 32.;
    let inverse = t.recip();
    let square = inverse * inverse;
    let correction = inverse
        * (1. / 12.
            + square
                * (-1. / 360. + square * (1. / 1260. + square * (-1. / 1680. + square / 1188.))));
    (t - 0.5) * t.ln() - t + 0.5 * (2. * std::f64::consts::PI).ln() + correction
        - (0..32).map(|j| (z + j as f64).ln()).sum::<f64>()
}
pub fn check_log_factorial() {
    for twice_y in 0..=40 {
        let y = twice_y as f64 * 0.5;
        let analytic = if twice_y % 2 == 0 {
            (1..=twice_y / 2).map(|i| (i as f64).ln()).sum()
        } else {
            0.5 * std::f64::consts::PI.ln()
                + (0..=twice_y / 2)
                    .map(|i| (i as f64 + 0.5).ln())
                    .sum::<f64>()
        };
        assert!((log_factorial(y) - analytic).abs() < 1e-12);
    }
}
fn weights(problem: &PpmlProblem) -> Vec<f64> {
    match &problem.weights {
        PpmlWeights::Unit => vec![1.; problem.outcome.len()],
        PpmlWeights::Frequency(v) => v.iter().map(|v| f64::from(*v)).collect(),
        PpmlWeights::Probability(v) => {
            let sum: f64 = v.iter().sum();
            v.iter()
                .map(|w| w * problem.outcome.len() as f64 / sum)
                .collect()
        }
    }
}
fn statistics(fit: &irls::Fit<'_>, problem: &PpmlProblem, expected: &PpmlResult, seed: usize) {
    let actual = statistics::ppml(fit).unwrap();
    let q = weights(problem);
    let sum_y: f64 = q.iter().zip(&problem.outcome).map(|(q, y)| q * y).sum();
    let mu0 = sum_y / q.iter().sum::<f64>();
    let factorial: f64 = q
        .iter()
        .zip(&problem.outcome)
        .map(|(q, y)| q * log_factorial(*y))
        .sum();
    let ll = expected.log_likelihood_kernel - factorial;
    let ll0: f64 = q
        .iter()
        .zip(&problem.outcome)
        .map(|(q, y)| q * (y * mu0.ln() - mu0 - log_factorial(*y)))
        .sum();
    close(actual.deviance, expected.deviance, seed);
    close(actual.log_likelihood, ll, seed);
    close(actual.null_mean, mu0, seed);
    close(actual.null_log_likelihood, ll0, seed);
    close(actual.pseudo_r_squared, 1. - ll / ll0, seed);
    close(actual.weighted_outcome_sum, sum_y, seed);
    assert_eq!(actual.outer_iterations, fit.diagnostics.accepted_iterations);
    assert_eq!(
        actual.inner_iterations,
        fit.diagnostics.work.counts().summed_iterations
    );
    assert_eq!(actual.step_halvings, fit.diagnostics.step_halvings);
    assert!(actual.converged);
    let legacy = fereg_core::ppml::PpmlLikelihoodStatistics::from_final_state(
        fereg_core::ppml::PoissonObjective {
            deviance: fit.evaluation.objective.deviance,
            log_likelihood: fit.evaluation.objective.log_likelihood,
        },
        &problem.outcome,
        &q,
    )
    .unwrap();
    close(actual.deviance, legacy.deviance, seed);
    close(actual.log_likelihood, legacy.log_likelihood, seed);
    close(actual.null_mean, legacy.null_mean, seed);
    close(actual.null_log_likelihood, legacy.null_log_likelihood, seed);
    close(actual.pseudo_r_squared, legacy.pseudo_r_squared, seed);
}
fn full_request(sample: &Prepared) -> PpmlRequest {
    PpmlRequest {
        common: Request {
            residuals: true,
            xb: true,
            xbd: true,
            absorbed: true,
            drop_reason: true,
            saved: sample
                .topology
                .as_ref()
                .unwrap()
                .effects()
                .iter()
                .enumerate()
                .flat_map(|(i, e)| {
                    (0..usize::from(e.intercept) + e.slopes.len()).map(move |j| (i, j))
                })
                .collect(),
        },
        mean: true,
        residual_type: None,
    }
}
fn scatter(sample: &Prepared, output: &Outputs) {
    for column in &output.columns {
        assert_eq!(column.values.len(), sample.n_original());
        for (&value, &reason) in column.values.iter().zip(&sample.drop_reasons) {
            if column.kind == Kind::DropReason {
                assert_eq!(value, f64::from(reason.code()));
            } else if reason == DropReason::Retained {
                assert!(value.is_finite());
            } else {
                assert!(value.is_nan());
            }
        }
    }
}
fn predictions(
    fit: &irls::Fit<'_>,
    problem: &PpmlProblem,
    expected: &PpmlResult,
    output: &Outputs,
    request: &PpmlRequest,
    seed: usize,
) {
    let input = fit.problem();
    let ordinary_constant = input.roles.last() == Some(&Role::Ordinary);
    for (i, &row) in input.sample.retained_rows().iter().enumerate() {
        let xb = problem.offset.get(i).copied().unwrap_or(0.)
            + problem.design[i][0] * expected.coefficients[0]
            + if ordinary_constant {
                expected.coefficients[1]
            } else {
                0.
            };
        close(output.get(Kind::Xb).unwrap()[row], xb, seed);
        close(output.get(Kind::Xbd).unwrap()[row], expected.eta[i], seed);
        close(output.get(Kind::Mean).unwrap()[row], expected.mean[i], seed);
        close(
            output.get(Kind::Absorbed).unwrap()[row],
            expected.eta[i] - xb,
            seed,
        );
        let saved_sum: f64 = request
            .common
            .saved
            .iter()
            .map(|&(effect, channel)| output.get(Kind::Saved { effect, channel }).unwrap()[row])
            .sum();
        close(saved_sum, output.get(Kind::Absorbed).unwrap()[row], seed);
        assert_eq!(output.get(Kind::Xbd).unwrap()[row], fit.point.eta[i]);
        assert_eq!(output.get(Kind::Mean).unwrap()[row], fit.evaluation.mean[i]);
    }
    scatter(input.sample, output);
}
fn residual_value(y: f64, mu: f64, kind: ResidualKind) -> f64 {
    let raw = y - mu;
    match kind {
        ResidualKind::Response | ResidualKind::Score => raw,
        ResidualKind::Pearson => raw / mu.sqrt(),
        ResidualKind::Working => raw / mu,
        ResidualKind::Deviance => {
            2. * if y == 0. {
                mu
            } else {
                mu - y + y * (y / mu).ln()
            }
        }
        ResidualKind::Anscombe => 1.5 * (y.powf(2. / 3.) - mu.powf(2. / 3.)) / mu.powf(1. / 6.),
    }
}
fn residuals(fit: &irls::Fit<'_>, expected: &PpmlResult, team: &ThreadTeam, seed: usize) {
    use fereg_core::ppml::PpmlResidualKind as Legacy;
    for (kind, legacy_kind) in [
        (ResidualKind::Response, Legacy::Response),
        (ResidualKind::Score, Legacy::Score),
        (ResidualKind::Pearson, Legacy::Pearson),
        (ResidualKind::Working, Legacy::Working),
        (ResidualKind::Deviance, Legacy::Deviance),
        (ResidualKind::Anscombe, Legacy::Anscombe),
    ] {
        let request = PpmlRequest {
            common: Request {
                residuals: true,
                ..Request::default()
            },
            residual_type: Some(kind),
            ..PpmlRequest::default()
        };
        let output = outputs::ppml(fit, &request, 1e-8, team).unwrap();
        assert_eq!(output.columns.len(), 1);
        scatter(fit.problem().sample, &output);
        let y = &fit.problem().sample.outcome;
        let legacy =
            fereg_core::ppml::ppml_residuals(y, &fit.evaluation.mean, legacy_kind).unwrap();
        for (i, &row) in fit.problem().sample.retained_rows().iter().enumerate() {
            let actual = output.get(Kind::Residuals).unwrap()[row];
            close(actual, residual_value(y[i], expected.mean[i], kind), seed);
            close(actual, legacy[i], seed);
        }
    }
}
pub fn compare(
    fit: &irls::Fit<'_>,
    problem: &PpmlProblem,
    expected: &PpmlResult,
    team: &ThreadTeam,
    seed: usize,
) {
    statistics(fit, problem, expected, seed);
    let request = full_request(fit.problem().sample);
    let before = statistics::ppml(fit).unwrap();
    let output = outputs::ppml(fit, &request, 1e-8, team)
        .unwrap_or_else(|e| panic!("outputs seed={seed}: {e}"));
    assert_eq!(output.columns.len(), 6 + request.common.saved.len());
    predictions(fit, problem, expected, &output, &request, seed);
    residuals(fit, expected, team, seed);
    assert_eq!(before, statistics::ppml(fit).unwrap());
}
pub fn identical(a: &irls::Fit<'_>, b: &irls::Fit<'_>, one: &ThreadTeam, eight: &ThreadTeam) {
    let request = full_request(a.problem().sample);
    let left = outputs::ppml(a, &request, 1e-8, one).unwrap();
    let right = outputs::ppml(b, &request, 1e-8, eight).unwrap();
    assert_eq!(left.certificate, right.certificate);
    for (a, b) in left.columns.iter().zip(&right.columns) {
        assert_eq!(a.kind, b.kind);
        assert_eq!(
            a.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            b.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    }
    assert_eq!(statistics::ppml(a).unwrap(), statistics::ppml(b).unwrap());
}
