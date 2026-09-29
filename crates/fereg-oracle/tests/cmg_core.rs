#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::DenseMatrix,
    fe::{CmgCore, Design, StopReason, Weights},
    threads::ThreadTeam,
    wls::{self, Policy, Role, Slot},
};
use fereg_oracle::{
    designs::{generate, Design as Generated, DesignSpec, Graph, WeightKind},
    explicit_ols::{fit_explicit_ols, ExplicitOlsProblem, OracleWeights},
};
#[path = "support/engine.rs"]
mod engine_inputs;
#[path = "support/outputs.rs"]
mod output_checks;

fn close(a: f64, b: f64, context: &str) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "{context}: {a:.17e} != {b:.17e}"
    );
}
fn check_design(generated: &Generated, weights: &[f64], context: &str, parallel: bool) {
    let sample = engine_inputs::prepare(&generated.effects, &generated.outcome);
    let design = Design::new(&sample, Weights::Provided(weights)).unwrap();
    let mut columns = vec![generated.outcome.clone()];
    columns.extend_from_slice(&generated.regressors);
    columns.push(vec![0.; sample.n_active()]);
    let rhs = DenseMatrix::from_columns(&columns).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let mut core = CmgCore::new(&design, [0, 1]).unwrap();
    let mut candidate = core.solve(&rhs, 16_000, 1e-12, &team).unwrap();
    assert!(
        candidate
            .progress
            .iter()
            .all(|p| p.stop == StopReason::Converged),
        "{context}: {:?}",
        candidate.progress
    );
    assert_eq!(candidate.progress.last().unwrap().iterations, 0);
    design
        .normalize_coefficients(&mut candidate.coefficients, 1e-10, &team)
        .unwrap();
    let residuals = design
        .reconstruct(&rhs, &candidate.coefficients, &team)
        .unwrap();
    design
        .certify(&rhs, &candidate.coefficients, &residuals, 1e-10, &team)
        .unwrap();
    if parallel {
        let eight = ThreadTeam::new(8).unwrap();
        let mut other = CmgCore::new(&design, [0, 1])
            .unwrap()
            .solve(&rhs, 16_000, 1e-12, &eight)
            .unwrap();
        design
            .normalize_coefficients(&mut other.coefficients, 1e-10, &eight)
            .unwrap();
        assert_eq!(candidate, other);
        let swapped = CmgCore::new(&design, [1, 0])
            .unwrap()
            .solve(&rhs, 16_000, 1e-12, &eight)
            .unwrap();
        let swapped = design
            .reconstruct(&rhs, &swapped.coefficients, &eight)
            .unwrap();
        for (a, b) in residuals.as_slice().iter().zip(swapped.as_slice()) {
            close(*a, *b, context);
        }
    }
    for (column, outcome) in columns.into_iter().enumerate() {
        let expected = fit_explicit_ols(&ExplicitOlsProblem {
            outcome,
            regressors: vec![],
            effects: generated.effects.clone(),
            weights: OracleWeights::Wls(weights.to_vec()),
            clusters: vec![],
        })
        .unwrap();
        if column == 0 {
            output_checks::check(
                &design,
                &candidate.coefficients,
                &residuals,
                &expected.residuals,
                &team,
            );
        }
        for (a, b) in residuals
            .column(column)
            .unwrap()
            .iter()
            .zip(&expected.residuals)
        {
            close(*a, *b, context);
        }
    }
    compare_finite(generated, weights, &residuals, context, &team);
}
fn compare_finite(
    generated: &Generated,
    weights: &[f64],
    residuals: &DenseMatrix,
    context: &str,
    team: &ThreadTeam,
) {
    let raw = DenseMatrix::from_columns(&generated.regressors).unwrap();
    let within = DenseMatrix::from_columns(
        &(0..raw.ncols())
            .map(|col| residuals.column(col + 1).unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let slots: Vec<_> = generated
        .regressors
        .iter()
        .map(|values| Slot {
            role: Role::Ordinary,
            reference_norm: values
                .iter()
                .zip(weights)
                .fold(0_f64, |norm, (x, w)| norm.hypot(w.sqrt() * x)),
        })
        .collect();
    let actual = wls::solve(
        &wls::Problem {
            design: &within,
            outcome: &residuals.column(0).unwrap(),
            weights: Weights::Provided(weights),
            slots: &slots,
            has_absorbed_intercept: true,
        },
        Policy::default(),
        team,
    )
    .unwrap();
    let expected = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: generated.outcome.clone(),
        regressors: generated.regressors.clone(),
        effects: generated.effects.clone(),
        weights: OracleWeights::Wls(weights.to_vec()),
        clusters: vec![],
    })
    .unwrap();
    for (a, b) in actual.coefficients.iter().zip(&expected.coefficients) {
        close(*a, b.unwrap_or(0.), context);
    }
    for (a, b) in actual.residuals.iter().zip(&expected.residuals) {
        close(*a, *b, context);
    }
    for (i, &left) in expected.kept_regressors.iter().enumerate() {
        for (j, &right) in expected.kept_regressors.iter().enumerate() {
            close(actual.bread[(left, right)], expected.bread[(i, j)], context);
        }
    }
}
fn campaign(start: usize, count: usize) {
    for seed in start..start + count {
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: 120 + seed % 71,
                levels: vec![9, 7],
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
        for frame in 0..2 {
            let weights: Vec<_> = generated
                .weights
                .iter()
                .enumerate()
                .map(|(i, w)| w * (1. + frame as f64 * (i % 7) as f64 * 0.17))
                .collect();
            check_design(
                &generated,
                &weights,
                &format!("seed {seed}, frame {frame}"),
                seed % 19 == 0,
            );
        }
    }
}
#[test]
fn cmg_core_projection_and_wls_match_dense_oracles() {
    campaign(0, 200);
}
#[test]
#[ignore = "long changing-weight CMG core campaign"]
fn two_thousand_cmg_core_designs() {
    campaign(200, 2000);
}

#[path = "support/counterexample.rs"]
mod regression;
#[test]
fn original_cmg_failure_matches_dense_projection_coefficients_and_bread() {
    let generated = regression::counterexample();
    check_design(
        &generated,
        &generated.weights,
        "800-row CMG regression",
        true,
    );
}
