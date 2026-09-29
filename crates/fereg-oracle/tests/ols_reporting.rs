#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::{
        ColumnRecipe, ColumnSlot, CompiledColumns, DenseMatrix, PreparedWeights, ReferenceWeights,
        SyntaxStatus, WeightKind as EngineWeight, Weights,
    },
    ols,
    spec::Estimator,
    threads::ThreadTeam,
    wls::{self, Policy, Problem, Role, Slot},
};
use fereg_oracle::{
    designs::{generate, DesignSpec, WeightKind},
    explicit_ols::{fit_explicit_ols, ExplicitOlsProblem, OracleWeights},
};
mod support;
fn columns(p: usize) -> CompiledColumns {
    let mut slots: Vec<_> = (0..p)
        .map(|j| ColumnSlot {
            name: format!("x{j}"),
            syntax: SyntaxStatus::Active,
            generated: Some(j),
        })
        .collect();
    slots.push(ColumnSlot {
        name: "_cons".into(),
        syntax: SyntaxStatus::Active,
        generated: Some(p),
    });
    let mut recipes: Vec<_> = (0..p)
        .map(|source| ColumnRecipe::Continuous { source })
        .collect();
    recipes.push(ColumnRecipe::Intercept);
    let mut roles = vec![Role::Ordinary; p];
    roles.push(Role::AbsorbedIntercept);
    CompiledColumns {
        slots,
        recipes,
        roles,
        constant_slot: Some(p),
        structural_intercept: true,
        implicit_constant: true,
    }
}
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() < 2e-9 * a.abs().max(b.abs()).max(1.),
        "seed {seed}: {a:.17e} != {b:.17e}"
    );
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let kind = [
            WeightKind::Unit,
            WeightKind::Analytic,
            WeightKind::Frequency,
            WeightKind::Probability,
        ][seed % 4];
        let g = generate(
            seed as u64,
            &DesignSpec {
                rows: 31 + seed % 67,
                levels: vec![],
                regressors: seed % 5,
                weights: kind,
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let n = g.outcome.len();
        let raw_weights = if kind == WeightKind::Unit {
            PreparedWeights::None
        } else {
            PreparedWeights::Provided {
                kind: match kind {
                    WeightKind::Analytic => EngineWeight::Analytic,
                    WeightKind::Frequency => EngineWeight::Frequency,
                    _ => EngineWeight::Probability,
                },
                values: g.weights.clone(),
            }
        };
        let weights = ReferenceWeights::new(&raw_weights, n, Estimator::Ols).unwrap();
        let mut raw_columns = g.regressors.clone();
        raw_columns.push(vec![1.; n]);
        let raw = DenseMatrix::from_columns(&raw_columns).unwrap();
        let within_columns: Vec<Vec<_>> = raw_columns
            .iter()
            .map(|col| {
                let mean = weights.mean(col).unwrap();
                col.iter().map(|x| x - mean).collect()
            })
            .collect();
        let within = DenseMatrix::from_columns(&within_columns).unwrap();
        let ymean = weights.mean(&g.outcome).unwrap();
        let within_y: Vec<_> = g.outcome.iter().map(|y| y - ymean).collect();
        let columns = columns(g.regressors.len());
        let slots: Vec<_> = columns
            .roles
            .iter()
            .enumerate()
            .map(|(j, &role)| Slot {
                role,
                reference_norm: (0..n).fold(0_f64, |norm, i| {
                    norm.hypot(weights.value(i).sqrt() * raw[(i, j)])
                }),
            })
            .collect();
        let problem = Problem {
            design: &within,
            outcome: &within_y,
            weights: Weights::Reference(weights),
            slots: &slots,
            has_absorbed_intercept: true,
        };
        let fit = wls::solve(&problem, Policy::default(), &one).unwrap();
        let actual = ols::report(&columns, &raw, &within, &g.outcome, weights, &fit).unwrap();
        if seed % 20 == 0 {
            let fit = wls::solve(&problem, Policy::default(), &eight).unwrap();
            assert_eq!(
                actual,
                ols::report(&columns, &raw, &within, &g.outcome, weights, &fit).unwrap()
            );
        }
        let expected = fit_explicit_ols(&ExplicitOlsProblem {
            outcome: g.outcome.clone(),
            regressors: raw_columns,
            effects: vec![],
            weights: OracleWeights::Wls((0..n).map(|i| weights.value(i)).collect()),
            clusters: vec![],
        })
        .unwrap();
        assert_eq!(actual.finite_rank, g.regressors.len());
        for (a, b) in actual.coefficients.iter().zip(&expected.coefficients) {
            close(*a, b.unwrap(), seed);
        }
        for (a, b) in actual
            .bread
            .as_slice()
            .iter()
            .zip(expected.bread.as_slice())
        {
            close(*a, *b, seed);
        }
        for (a, b) in fit.residuals.iter().zip(&expected.residuals) {
            close(*a, *b, seed);
        }
        let (legacy, _) =
            support::legacy_fit(&g, fereg_core::PreparedOlsFitConfig::default(), true).unwrap();
        assert_eq!(legacy.finite_rank(), actual.finite_rank);
        for (a, b) in actual.coefficients.iter().zip(legacy.coefficients()) {
            close(*a, b.value.unwrap(), seed);
        }
        for (a, b) in actual.bread.as_slice().iter().zip(legacy.bread().values()) {
            close(*a, *b, seed);
        }
    }
}
#[test]
fn reported_coordinates_match_dense_oracle_and_pinned_legacy() {
    campaign(0, 200);
}
#[test]
#[ignore = "long normalized-constant oracle and legacy differential campaign"]
fn two_thousand_reported_coordinate_designs() {
    campaign(200, 2000);
}
