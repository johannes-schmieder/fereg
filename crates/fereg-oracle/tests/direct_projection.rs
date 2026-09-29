#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    fe::{Design, Weights},
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};
use fereg_oracle::{
    designs::{generate, DesignSpec, WeightKind},
    explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights},
};
fn prepare(effect: &DummyEffect, outcome: &[f64]) -> Prepared {
    let mut sources = vec![effect.codes().iter().map(|&code| f64::from(code)).collect()];
    sources.extend_from_slice(effect.slopes());
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
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).unwrap(),
            categorical_sources: vec![SourceId::new(1).unwrap()],
            intercept: effect.has_intercept(),
            slope_sources: (1..sources.len())
                .map(|i| SourceId::new(i as u32 + 1).unwrap())
                .collect(),
            label: "effect".into(),
            saved_name: String::new(),
        }],
        clusters: vec![],
        include_intercept: effect.has_intercept(),
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
    let single = ThreadTeam::new(1).unwrap();
    let multi = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: [65, 120, 191][seed % 3],
                levels: vec![3 + seed % 5],
                slopes: true,
                regressors: 2,
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
        let initial = &generated.effects[0];
        let slope = initial.slopes()[0].clone();
        let scale = [1e-100, 1., 1e100][seed % 3];
        let slopes = match seed % 5 {
            0 => vec![],
            1 => vec![slope],
            2 => vec![
                slope.clone(),
                slope.iter().map(|x| x * scale).collect(),
                vec![0.; slope.len()],
            ],
            3 => vec![
                slope.clone(),
                generated.regressors[0].clone(),
                vec![1.; slope.len()],
            ],
            _ => vec![slope.iter().map(|x| x * scale).collect()],
        };
        let intercept = slopes.is_empty() || seed % 2 == 0;
        let effect = DummyEffect::new(
            initial.codes().to_vec(),
            initial.n_levels(),
            intercept,
            slopes,
        )
        .unwrap();
        let sample = prepare(&effect, &generated.outcome);
        let design = Design::new(&sample, Weights::Provided(&generated.weights)).unwrap();
        let mut columns = vec![generated.outcome.clone()];
        columns.extend_from_slice(&generated.regressors);
        let rhs = DenseMatrix::from_columns(&columns).unwrap();
        let result = design.direct_candidate(&rhs, &single).unwrap();
        assert_eq!(
            result,
            design.direct_candidate(&rhs, &multi).unwrap(),
            "seed {seed}: worker determinism"
        );
        design
            .certify(
                &rhs,
                &result.coefficients,
                &result.residuals,
                2e-10,
                &single,
            )
            .unwrap_or_else(|e| panic!("certificate seed {seed}: {e}"));
        for (column, outcome) in columns.into_iter().enumerate() {
            let expected = fit_explicit_ols(&ExplicitOlsProblem {
                outcome,
                regressors: vec![],
                effects: vec![effect.clone()],
                weights: OracleWeights::Wls(generated.weights.clone()),
                clusters: vec![],
            })
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            assert_eq!(
                result.level_ranks.iter().sum::<usize>(),
                expected.explicit_dummy_rank,
                "seed {seed}: rank"
            );
            for (row, &expected) in expected.residuals.iter().enumerate() {
                let actual = result.residuals[(row, column)];
                assert!(
                    (actual - expected).abs() < 2e-10 * expected.abs().max(1.),
                    "seed {seed}, row {row}, RHS {column}: {actual} != {expected}"
                );
            }
        }
    }
}
#[test]
fn direct_projection_matches_independent_dense_dummy_qr() {
    campaign(0, 200);
}
#[test]
#[ignore = "long direct weighted-span campaign"]
fn two_thousand_direct_projection_designs() {
    campaign(200, 2000);
}
