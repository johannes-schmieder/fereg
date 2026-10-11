#![deny(clippy::too_many_lines)]
use fereg_oracle::compare::{Tolerance, COEFFICIENT};
use fereg_oracle::designs::{generate, DesignSpec, Graph, WeightKind};
use fereg_oracle::explicit_ols::fit_explicit_ols;
use fereg_oracle::explicit_ppml::fit_explicit_ppml;

#[test]
fn campaign_topology_has_consecutive_workers_and_persistent_random_destinations() {
    let spec = DesignSpec {
        rows: 10_003,
        levels: vec![2001, 100],
        graph: Graph::AkmCampaign,
        regressors: 0,
        ..DesignSpec::default()
    };
    let design = generate(739391, &spec).unwrap();
    assert_eq!(design, generate(739391, &spec).unwrap());
    let worker = design.effects[0].codes();
    let firm = design.effects[1].codes();
    let mut moves = 0;
    let mut nonadjacent = 0;
    for row in 0..spec.rows {
        assert_eq!(worker[row] as usize, row / 5);
        assert!(firm[row] < 100);
        if row % 5 == 0 {
            assert_eq!(firm[row] as usize, (row / 5) % 100);
        } else if firm[row] != firm[row - 1] {
            moves += 1;
            nonadjacent += usize::from(firm[row].abs_diff(firm[row - 1]) > 1);
        }
    }
    assert!((1400..1800).contains(&moves));
    assert!(nonadjacent > 1000);
    let mut invalid = spec;
    invalid.levels[0] -= 1;
    assert!(generate(739391, &invalid).is_err());
    invalid.levels.clear();
    assert!(generate(739391, &invalid).is_err());
}

#[test]
fn seeded_designs_cover_graphs_weights_slopes_missingness_and_collinearity() {
    for (j, graph) in [
        Graph::DensePanel,
        Graph::AkmStrong,
        Graph::AkmWeak,
        Graph::Chain,
        Graph::Barbell,
        Graph::Disconnected,
        Graph::RandomMobility,
    ]
    .into_iter()
    .enumerate()
    {
        for weights in [
            WeightKind::Unit,
            WeightKind::Analytic,
            WeightKind::Frequency,
            WeightKind::Probability,
        ] {
            let spec = DesignSpec {
                rows: 240,
                levels: vec![6, 6, 4],
                graph,
                weights,
                slopes: true,
                missing_fraction: 0.05,
                singleton: true,
                collinear: true,
                clusters: 2,
                ..DesignSpec::default()
            };
            let design = generate(j as u64, &spec).unwrap();
            assert_eq!(design, generate(j as u64, &spec).unwrap());
            assert_ne!(design, generate(j as u64 + 99, &spec).unwrap());
            let (problem, rows) = design.oracle_problem(false).unwrap();
            assert!(rows.len() < spec.rows);
            let fit = fit_explicit_ols(&problem).unwrap();
            assert_eq!(fit.finite_regressor_rank, spec.regressors);
            assert_eq!(fit.coefficients.iter().filter(|b| b.is_none()).count(), 1);
        }
    }
}

#[test]
fn singleton_pruning_uses_frequency_multiplicity() {
    let spec = DesignSpec {
        singleton: true,
        ..DesignSpec::default()
    };
    let mut design = generate(42, &spec).unwrap();
    let (_, rows) = design.oracle_problem(false).unwrap();
    assert!(!rows.contains(&(spec.rows - 1)));
    design.weight_kind = WeightKind::Frequency;
    design.weights[spec.rows - 1] = 2.0;
    let (_, rows) = design.oracle_problem(false).unwrap();
    assert!(rows.contains(&(spec.rows - 1)));
}

#[test]
fn generated_poisson_models_include_zeros_and_finite_explicit_solutions() {
    for weights in [
        WeightKind::Unit,
        WeightKind::Frequency,
        WeightKind::Probability,
    ] {
        let spec = DesignSpec {
            poisson: true,
            zero_fraction: 0.15,
            weights,
            ..DesignSpec::default()
        };
        let design = generate(12, &spec).unwrap();
        assert!(design.outcome.contains(&0.0));
        let (problem, _) = design.poisson_problem(false).unwrap();
        let fit = fit_explicit_ppml(&problem).unwrap();
        assert!(fit.mean.iter().all(|m| m.is_finite() && *m > 0.0));
    }
}

#[test]
fn comparison_helpers_reject_nonfinite_mismatched_or_inaccurate_outputs() {
    for value in [f64::NAN, f64::INFINITY, 1.01] {
        assert!(COEFFICIENT.check(value, 1.0).is_err());
    }
    assert!(COEFFICIENT.check_slice(&[1.0], &[]).is_err());
    Tolerance {
        relative: 0.0,
        absolute: 1e-10,
    }
    .check(0.5e-10, 0.0)
    .unwrap();
}
