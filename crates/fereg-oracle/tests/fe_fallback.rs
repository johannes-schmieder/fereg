use fereg_engine::{
    data::DenseMatrix,
    fe::{Backend, Design, Engine, FallbackReason, MonitorControl, PlanOverride, Weights},
    spec::{Estimator, Options},
    threads::ThreadTeam,
};
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph, WeightKind},
    explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights},
};
#[path = "support/engine.rs"]
mod engine_inputs;

fn check(
    effects: &[DummyEffect],
    outcome: &[f64],
    weights: &[f64],
    budget: usize,
    monitor: MonitorControl,
) {
    let sample = engine_inputs::prepare(effects, outcome);
    let design = Design::new(&sample, Weights::Provided(weights)).unwrap();
    let rhs = DenseMatrix::from_columns(&[outcome.to_vec(), vec![0.; outcome.len()]]).unwrap();
    let options = Options {
        iterations: budget,
        tolerance: 1e-10,
        ..Options::for_estimator(Estimator::Ols)
    };
    let expected = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: outcome.to_vec(),
        regressors: vec![],
        effects: effects.to_vec(),
        weights: OracleWeights::Wls(weights.to_vec()),
        clusters: vec![],
    })
    .unwrap();
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let actual = Engine::with_controls(
            &design,
            Estimator::Ols,
            &options,
            PlanOverride::Automatic,
            monitor,
            &team,
        )
        .unwrap()
        .project(&rhs, &team)
        .unwrap();
        let report = &actual.diagnostics[0];
        assert_eq!(report.used, Backend::Lsmr);
        assert_eq!(report.attempts.len(), 2);
        assert!(matches!(
            report.fallback,
            Some(FallbackReason::MeasuredStagnation | FallbackReason::ProjectedExhaustion)
        ));
        assert!(report.iterations <= budget);
        assert_eq!(actual.diagnostics[1].used, Backend::SchurPcg);
        assert_eq!(actual.diagnostics[1].iterations, 0);
        for (row, &oracle) in expected.residuals.iter().enumerate() {
            assert!((actual.residuals[(row, 0)] - oracle).abs() <= 2e-9 * oracle.abs().max(1.));
        }
        if let Some(expected) = &reference {
            assert_eq!(expected, &actual);
        } else {
            reference = Some(actual);
        }
    }
}

#[test]
fn measured_exhaustion_on_a_weak_graph_matches_independent_dense_qr() {
    let n = 202;
    let effects: Vec<_> = [101, 100]
        .into_iter()
        .map(|levels| {
            DummyEffect::new(
                (0..n).map(|i| (i % levels) as u32).collect(),
                levels,
                true,
                vec![],
            )
            .unwrap()
        })
        .collect();
    let outcome: Vec<_> = (0..n).map(|i| (i as f64 * 0.13).sin()).collect();
    check(
        &effects,
        &outcome,
        &vec![1.; n],
        32,
        MonitorControl::Automatic,
    );
}

fn campaign(start: usize, count: usize) {
    for seed in start..start + count {
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: 130 + seed % 61,
                levels: vec![11, 7, 3],
                slopes: seed % 3 == 0,
                graph: [
                    Graph::DensePanel,
                    Graph::AkmStrong,
                    Graph::AkmWeak,
                    Graph::Chain,
                    Graph::Barbell,
                    Graph::Disconnected,
                ][seed % 6],
                weights: [
                    WeightKind::Unit,
                    WeightKind::Analytic,
                    WeightKind::Frequency,
                    WeightKind::Probability,
                ][seed % 4],
                ..DesignSpec::default()
            },
        )
        .unwrap();
        check(
            &generated.effects,
            &generated.outcome,
            &generated.weights,
            1000,
            MonitorControl::StopAfter(1),
        );
    }
}
#[test]
fn per_rhs_transfer_matches_independent_dense_qr() {
    campaign(0, 200);
}
#[test]
#[ignore = "long retained-backend transfer campaign"]
fn two_thousand_transfer_designs() {
    campaign(200, 2000);
}
