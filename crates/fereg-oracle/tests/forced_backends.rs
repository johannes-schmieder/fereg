#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::DenseMatrix,
    error::ErrorCode,
    fe::{Backend, Design, Engine, MonitorControl, Plan, PlanOverride, Weights},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
};
use fereg_oracle::{
    designs::{generate, Design as Generated, DesignSpec, Graph, WeightKind},
    explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights},
};
#[path = "support/engine.rs"]
mod engine_inputs;

fn input(seed: usize) -> Generated {
    let mut generated = generate(
        seed as u64,
        &DesignSpec {
            rows: 130 + seed % 61,
            levels: [9, 7, 3][..seed % 4].to_vec(),
            graph: [
                Graph::DensePanel,
                Graph::AkmStrong,
                Graph::AkmWeak,
                Graph::Chain,
                Graph::Barbell,
                Graph::Disconnected,
            ][(seed / 4) % 6],
            slopes: (seed / 24) % 2 == 1,
            weights: [
                WeightKind::Unit,
                WeightKind::Analytic,
                WeightKind::Frequency,
                WeightKind::Probability,
            ][(seed / 48) % 4],
            ..DesignSpec::default()
        },
    )
    .unwrap();
    if let Some(effect) = generated.effects.first_mut() {
        if let Some(slope) = effect.slopes().first() {
            *effect = DummyEffect::new(
                effect.codes().to_vec(),
                effect.n_levels(),
                seed % 5 != 0,
                vec![
                    slope.clone(),
                    slope.iter().map(|x| 2. * x).collect(),
                    vec![0.; slope.len()],
                ],
            )
            .unwrap();
        }
        if seed % 7 == 0 {
            generated.effects.push(generated.effects[0].clone());
        }
    }
    generated
}

fn campaign(start: usize, count: usize) {
    let teams: Vec<_> = [1, 2, 4, 8]
        .into_iter()
        .map(|n| ThreadTeam::new(n).unwrap())
        .collect();
    let options = Options {
        tolerance: 1e-10,
        iterations: 1000,
        ..Options::for_estimator(Estimator::Ols)
    };
    for seed in start..start + count {
        let generated = input(seed);
        let prepared = engine_inputs::prepare(&generated.effects, &generated.outcome);
        let design = Design::new(&prepared, Weights::Provided(&generated.weights)).unwrap();
        let columns = vec![
            generated.outcome.clone(),
            generated.regressors[0].clone(),
            vec![0.; generated.outcome.len()],
        ];
        let rhs = DenseMatrix::from_columns(&columns).unwrap();
        let expected: Vec<_> = columns
            .iter()
            .take(2)
            .map(|outcome| {
                fit_explicit_ols(&ExplicitOlsProblem {
                    outcome: outcome.clone(),
                    regressors: vec![],
                    effects: generated.effects.clone(),
                    weights: OracleWeights::Wls(generated.weights.clone()),
                    clusters: vec![],
                })
                .unwrap()
                .residuals
            })
            .collect();
        for backend in [
            Backend::NoEffects,
            Backend::Direct,
            Backend::SchurPcg,
            Backend::Lsmr,
            Backend::Cmg,
        ] {
            let override_plan = PlanOverride::Force(backend);
            if let Err(error) =
                Plan::with_override(&design, Estimator::Ols, &options, override_plan)
            {
                assert_eq!(error.code, ErrorCode::Unsupported);
                continue;
            }
            let mut reference = None;
            for team in &teams {
                let actual = Engine::with_controls(
                    &design,
                    Estimator::Ols,
                    &options,
                    override_plan,
                    MonitorControl::Disabled,
                    team,
                )
                .unwrap()
                .project(&rhs, team)
                .unwrap_or_else(|error| panic!("seed {seed}, {backend:?}: {error}"));
                for report in &actual.diagnostics {
                    assert_eq!(report.requested, Solver::Auto);
                    assert_eq!(report.selected, backend);
                    assert_eq!(report.attempts.len(), 1, "seed {seed}, {backend:?}");
                    assert!(report.fallback.is_none());
                    assert!(report.iterations <= options.iterations);
                }
                assert_eq!(actual.diagnostics[2].iterations, 0);
                for (column, residuals) in expected.iter().enumerate() {
                    for (row, &oracle) in residuals.iter().enumerate() {
                        assert!(
                            (actual.residuals[(row, column)] - oracle).abs()
                                <= 2e-9 * oracle.abs().max(1.),
                            "seed {seed}, {backend:?}, RHS {column}, row {row}: actual={} oracle={oracle}; score={:?}; work={:?}",
                            actual.residuals[(row, column)], actual.certificate.relative_score,
                            actual.diagnostics
                        );
                    }
                }
                if let Some(reference) = &reference {
                    assert_eq!(&actual, reference, "seed {seed}, {backend:?}");
                } else {
                    reference = Some(actual);
                }
            }
        }
    }
}

#[test]
fn every_supported_override_matches_independent_dense_qr() {
    campaign(0, 200);
}

#[test]
#[ignore = "long all-backend dispatch and worker-identity campaign"]
fn two_thousand_forced_backend_designs() {
    campaign(200, 2000);
}

#[test]
fn true_residual_corrections_resolve_weak_direction_error_at_unchanged_tolerance() {
    use fereg_engine::fe::{LsmrSession, StopReason};
    let team = ThreadTeam::new(1).unwrap();
    for seed in [59, 274] {
        let generated = input(seed);
        let prepared = engine_inputs::prepare(&generated.effects, &generated.outcome);
        let design = Design::new(&prepared, Weights::Provided(&generated.weights)).unwrap();
        let rhs = DenseMatrix::from_columns(&[
            generated.outcome.clone(),
            generated.regressors[0].clone(),
        ])
        .unwrap();
        let session = LsmrSession::new(&design, &team).unwrap();
        let first = session.solve(&rhs, 1000, 1e-10, &team).unwrap();
        let residual = design
            .reconstruct(&rhs, &first.coefficients, &team)
            .unwrap();
        // This local-span certificate passes before observation-space accuracy
        // meets the stronger campaign check. It is not a forward-error bound.
        design
            .certify(&rhs, &first.coefficients, &residual, 1e-10, &team)
            .unwrap();
        let refined = session.solve_refined(&rhs, 1000, 1e-10, &team).unwrap();
        let accepted = design
            .reconstruct(&rhs, &refined.coefficients, &team)
            .unwrap();
        let mut exposed = false;
        for column in 0..rhs.ncols() {
            let oracle = fit_explicit_ols(&ExplicitOlsProblem {
                outcome: rhs.column(column).unwrap(),
                regressors: vec![],
                effects: generated.effects.clone(),
                weights: OracleWeights::Wls(generated.weights.clone()),
                clusters: vec![],
            })
            .unwrap();
            let before = oracle
                .residuals
                .iter()
                .enumerate()
                .map(|(row, &x)| (residual[(row, column)] - x).abs())
                .fold(0., f64::max);
            let after = oracle
                .residuals
                .iter()
                .enumerate()
                .map(|(row, &x)| (accepted[(row, column)] - x).abs())
                .fold(0., f64::max);
            exposed |= before > 2e-9;
            assert!(after < 1e-9, "seed {seed}, RHS {column}: {after:e}");
            assert_eq!(refined.progress[column].stop, StopReason::Converged);
            assert!(refined.progress[column].iterations > first.progress[column].iterations);
            assert!(refined.progress[column].refinements > 0);
        }
        assert!(
            exposed,
            "seed {seed} no longer exposes the first-stop discrepancy"
        );
        let budget = first.progress.iter().map(|a| a.iterations).max().unwrap();
        let limited = session.solve_refined(&rhs, budget, 1e-10, &team).unwrap();
        assert!(limited.progress.iter().all(|a| a.iterations <= budget));
        assert!(limited
            .progress
            .iter()
            .any(|a| a.stop == StopReason::BudgetExhausted));
    }
}
