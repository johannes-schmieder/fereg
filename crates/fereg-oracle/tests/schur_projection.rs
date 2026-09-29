#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    fe::{Design, Plan, SchurSession, StopReason, Weights},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph, WeightKind},
    explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights},
};
fn prepare(effects: &[DummyEffect], outcome: &[f64]) -> Prepared {
    let mut sources = Vec::new();
    let mut terms = Vec::new();
    for (index, effect) in effects.iter().enumerate() {
        sources.push(effect.codes().iter().map(|&code| f64::from(code)).collect());
        let category = SourceId::new(sources.len() as u32).unwrap();
        let mut slopes = Vec::new();
        for slope in effect.slopes() {
            sources.push(slope.clone());
            slopes.push(SourceId::new(sources.len() as u32).unwrap());
        }
        terms.push(AbsorbedTerm {
            id: AbsorbTermId::new(index as u32 + 1).unwrap(),
            categorical_sources: vec![category],
            intercept: effect.has_intercept(),
            slope_sources: slopes,
            label: format!("g{index}"),
            saved_name: String::new(),
        });
    }
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (0..sources.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: terms,
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    prepare_fixed_effects(
        &model,
        Prepared {
            topology: None,
            n_original: outcome.len(),
            n_candidate: outcome.len(),
            original_row_by_active: (0..outcome.len()).collect(),
            outcome: outcome.to_vec(),
            regressors: sources,
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; outcome.len()],
        },
    )
    .unwrap()
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: [120, 191][seed % 2],
                levels: if seed % 3 == 0 {
                    vec![8, 5, 3]
                } else {
                    vec![9, 7]
                },
                graph: [
                    Graph::DensePanel,
                    Graph::AkmStrong,
                    Graph::AkmWeak,
                    Graph::Chain,
                    Graph::Barbell,
                    Graph::Disconnected,
                ][seed % 6],
                slopes: seed % 4 == 0,
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
        let mut effects = generated.effects.clone();
        if seed % 4 == 0 {
            let effect = &effects[0];
            let slope = effect.slopes()[0].clone();
            effects[0] = DummyEffect::new(
                effect.codes().to_vec(),
                effect.n_levels(),
                seed % 8 == 0,
                vec![
                    slope.clone(),
                    slope.iter().map(|v| 2. * v).collect(),
                    vec![0.; slope.len()],
                ],
            )
            .unwrap();
        }
        if seed % 7 == 0 {
            effects.push(effects[0].clone());
        }
        let sample = prepare(&effects, &generated.outcome);
        let design = Design::new(&sample, Weights::Provided(&generated.weights)).unwrap();
        let options = Options {
            tolerance: 1e-12,
            iterations: 1000,
            ..Options::for_estimator(Estimator::Ols)
        };
        let plan = Plan::new(&design, Estimator::Ols, &options).unwrap();
        let columns = vec![
            generated.outcome.clone(),
            generated.regressors[0].clone(),
            vec![0.; sample.n_active()],
        ];
        let rhs = DenseMatrix::from_columns(&columns).unwrap();
        let result = SchurSession::new(&design, &plan, &one)
            .unwrap()
            .solve(&rhs, &one)
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        assert!(
            result
                .progress
                .iter()
                .all(|p| p.stop == StopReason::Converged),
            "seed {seed}: {:?}",
            result.progress
        );
        assert_eq!(result.progress[2].iterations, 0);
        if seed % 10 == 0 {
            assert_eq!(
                result,
                SchurSession::new(&design, &plan, &eight)
                    .unwrap()
                    .solve(&rhs, &eight)
                    .unwrap(),
                "seed {seed}: worker identity"
            );
        }
        for (column, outcome) in columns.into_iter().take(2).enumerate() {
            let expected = fit_explicit_ols(&ExplicitOlsProblem {
                outcome,
                regressors: vec![],
                effects: effects.clone(),
                weights: OracleWeights::Wls(generated.weights.clone()),
                clusters: vec![],
            })
            .unwrap_or_else(|e| panic!("oracle seed {seed}: {e}"));
            for (row, &expected) in expected.residuals.iter().enumerate() {
                let actual = result.residuals[(row, column)];
                assert!(
                    (actual - expected).abs() < 2e-9 * expected.abs().max(1.),
                    "seed {seed}, row {row}, RHS {column}: {actual} != {expected}"
                );
            }
        }
    }
}
#[test]
fn schur_projection_matches_independent_dense_dummy_qr() {
    campaign(0, 200);
}
#[test]
#[ignore = "long Schur-PCG weighted-span campaign"]
fn two_thousand_schur_projection_designs() {
    campaign(200, 2000);
}
