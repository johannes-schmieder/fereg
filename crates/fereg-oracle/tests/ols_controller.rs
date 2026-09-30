//! Complete public Rust calls, compared with independent full-dummy fits.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    memory::Inputs,
    ols::{self, Call, Request, Resources},
    outputs::{self, Kind},
    spec::{ClusterTerm, Estimator, Options, Solver},
    threads::ThreadTeam,
    vce, *,
};
use fereg_oracle::{
    designs::{generate, Design, DesignSpec, Graph, WeightKind as OracleWeight},
    explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights},
};
fn column(values: Vec<f64>) -> NumericColumn {
    NumericColumn {
        values,
        missing: None,
    }
}
fn source(index: usize) -> SourceId {
    SourceId::new(index as u32 + 1).unwrap()
}
fn term(index: usize) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(index as u32 + 1).unwrap(),
        order: TermOrder::new(index as u32 + 1).unwrap(),
        coefficient_slot: CoefficientSlot::new(index as u32 + 1).unwrap(),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: format!("x{index}"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).unwrap(),
            source: source(index),
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.,
            base: MetadataFlag::False,
            omitted: MetadataFlag::False,
            raw_operator: "c".into(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}
fn input(g: &Design) -> (ModelSpec, Loaded) {
    let mut values = g.regressors.clone();
    let effects = g
        .effects
        .iter()
        .enumerate()
        .map(|(index, effect)| {
            let categorical = source(values.len());
            values.push(effect.codes().iter().map(|&c| c as f64).collect());
            let slopes = effect
                .slopes()
                .iter()
                .map(|slope| {
                    let id = source(values.len());
                    values.push(slope.clone());
                    id
                })
                .collect();
            AbsorbedTerm {
                id: AbsorbTermId::new(index as u32 + 1).unwrap(),
                intercept: effect.has_intercept(),
                categorical_sources: vec![categorical],
                slope_sources: slopes,
                label: format!("g{index}"),
                saved_name: String::new(),
            }
        })
        .collect();
    let clusters = g
        .clusters
        .iter()
        .enumerate()
        .map(|(index, codes)| {
            let id = source(values.len());
            values.push(codes.iter().map(|&c| c as f64).collect());
            ClusterTerm {
                label: format!("c{index}"),
                sources: vec![id],
            }
        })
        .collect();
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (0..values.len())
                .map(|index| SourceEntry {
                    id: source(index),
                    canonical_name: format!("s{index}"),
                })
                .collect(),
            terms: (0..g.regressors.len()).map(term).collect(),
        },
        absorbed_terms: effects,
        clusters,
        include_intercept: true,
        keep_singletons: true,
    };
    let kind = match g.weight_kind {
        OracleWeight::Unit => None,
        OracleWeight::Analytic => Some(WeightKind::Analytic),
        OracleWeight::Frequency => Some(WeightKind::Frequency),
        OracleWeight::Probability => Some(WeightKind::Probability),
    };
    let loaded = Loaded {
        candidate: vec![true; g.outcome.len()],
        outcome: column(g.outcome.clone()),
        sources: values
            .into_iter()
            .map(|v| LoadedSource::Numeric(column(v)))
            .collect(),
        weights: kind.map(|kind| (kind, column(g.weights.clone()))),
        adjustment: LoadedAdjustment::None,
    };
    (model, loaded)
}
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "seed {seed}: {a:.17e} != {b:.17e}"
    );
}
fn oracle(g: &Design) -> fereg_oracle::explicit_ols::ExplicitOlsResult {
    let sum = g.weights.iter().sum::<f64>();
    let mut effects = g.effects.clone();
    if effects.is_empty() {
        effects.push(DummyEffect::new(vec![0; g.outcome.len()], 1, true, vec![]).unwrap());
    }
    fit_explicit_ols(&ExplicitOlsProblem {
        outcome: g.outcome.clone(),
        regressors: g.regressors.clone(),
        effects,
        weights: match g.weight_kind {
            OracleWeight::Unit => OracleWeights::Unit,
            OracleWeight::Frequency => {
                OracleWeights::Frequency(g.weights.iter().map(|&w| w as u32).collect())
            }
            _ => OracleWeights::Wls(
                g.weights
                    .iter()
                    .map(|w| w * g.outcome.len() as f64 / sum)
                    .collect(),
            ),
        },
        clusters: g.clusters.clone(),
    })
    .unwrap()
}
fn compare(fit: &ols::OlsFit, g: &Design, seed: usize) {
    let expected = oracle(g);
    let (absorbed, nested) = nesting_counts(g, expected.explicit_dummy_rank);
    let structural = expected.effective_observations - expected.finite_regressor_rank - absorbed;
    let correction = structural - usize::from(nested > 0);
    assert_eq!(
        fit.dof.finite, expected.finite_regressor_rank,
        "seed {seed}"
    );
    assert_eq!(fit.dof.absorbed, absorbed, "seed {seed}");
    assert_eq!(fit.dof.structural, structural, "seed {seed}");
    assert_eq!(fit.dof.nested, nested, "seed {seed}");
    assert_eq!(fit.dof.fit, structural - nested, "seed {seed}");
    assert_eq!(fit.dof.correction, correction, "seed {seed}");
    assert_eq!(
        fit.sample.retained_rows,
        (0..g.outcome.len()).collect::<Vec<_>>()
    );
    assert!(fit
        .sample
        .drop_reasons
        .iter()
        .all(|r| *r == DropReason::Retained));
    for (slot, coefficient) in expected.coefficients.iter().enumerate() {
        if let Some(value) = coefficient {
            close(fit.coefficients[slot], *value, seed);
        } else {
            assert_eq!(fit.coefficients[slot], 0.);
        }
    }
    let cons = (0..g.outcome.len())
        .map(|row| {
            g.weights[row]
                * (g.outcome[row]
                    - (0..g.regressors.len())
                        .map(|j| g.regressors[j][row] * expected.coefficients[j].unwrap_or(0.))
                        .sum::<f64>())
        })
        .sum::<f64>()
        / g.weights.iter().sum::<f64>();
    close(*fit.coefficients.last().unwrap(), cons, seed);
    close(fit.statistics.rss, expected.weighted_rss, seed);
    let residual = fit.outputs.get(Kind::Residuals).unwrap();
    let xbd = fit.outputs.get(Kind::Xbd).unwrap();
    for row in 0..g.outcome.len() {
        close(residual[row], expected.residuals[row], seed);
        close(xbd[row], expected.fitted_values[row], seed);
    }
    let covariance = if g.clusters.is_empty() {
        expected.normalized_reported_intercept_hc1_vce.unwrap()
    } else {
        expected
            .normalized_reported_intercept_clustered_vce
            .unwrap()
    };
    // The independent full-model sandwich has its exact model-rank correction.
    // Replace that scalar once with the separately checked nesting allocation.
    let scale = if g.clusters.is_empty() {
        1.
    } else {
        expected.residual_degrees_of_freedom as f64 / correction as f64
    };
    for (i, &left) in fit.coordinates.iter().enumerate() {
        for (j, &right) in fit.coordinates.iter().enumerate() {
            close(
                fit.covariance.values[(left, right)],
                covariance[(i, j)] * scale,
                seed,
            );
        }
    }
}
fn nesting_counts(g: &Design, full_rank: usize) -> (usize, usize) {
    use std::collections::{BTreeMap, BTreeSet};
    let mut nested = 0;
    let mut eligible = 0;
    let mut slopes = 0;
    for effect in &g.effects {
        let levels = effect.codes().iter().collect::<BTreeSet<_>>().len();
        slopes += levels * effect.slopes().len();
        let is_nested = g.clusters.iter().any(|cluster| {
            let mut assignments = BTreeMap::new();
            effect.codes().iter().zip(cluster).all(|(&level, &group)| {
                assignments
                    .insert(level, group)
                    .is_none_or(|previous| previous == group)
            })
        });
        if effect.has_intercept() {
            if is_nested {
                nested += levels;
            } else {
                eligible += levels;
            }
        }
    }
    // At most two effects: with nesting, at most one intercept partition
    // remains, whose first direction is redundant under the pinned policy.
    (
        if nested == 0 {
            full_rank
        } else {
            slopes + eligible.saturating_sub(1)
        },
        nested,
    )
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let slopes = seed % 5 == 0;
        let g = generate(
            seed as u64,
            &DesignSpec {
                rows: 97 + seed % 37,
                levels: vec![4, 5][..seed % 3].to_vec(),
                regressors: 1 + seed % 2,
                slopes,
                weights: [
                    OracleWeight::Unit,
                    OracleWeight::Analytic,
                    OracleWeight::Frequency,
                    OracleWeight::Probability,
                ][seed % 4],
                graph: if seed % 3 == 2 && seed % 7 == 0 {
                    Graph::Disconnected
                } else {
                    Graph::DensePanel
                },
                clusters: usize::from(seed % 2 == 0),
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let (model, loaded) = input(&g);
        let request = Request {
            vce: Some(if g.clusters.is_empty() {
                vce::Kind::Robust
            } else {
                vce::Kind::Cluster
            }),
            outputs: outputs::Request {
                residuals: true,
                xbd: true,
                ..outputs::Request::default()
            },
            ..Request::default()
        };
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            let options = Options {
                solver,
                threads: 1,
                tolerance: 1e-12,
                ..Options::for_estimator(Estimator::Ols)
            };
            let make = |threads| {
                Call::new(
                    &model,
                    &Options { threads, ..options },
                    &request,
                    Inputs::from_loaded(&loaded, 0, 0).unwrap(),
                    loaded.weights.as_ref().map(|(kind, _)| *kind),
                    Resources::default(),
                )
                .unwrap()
            };
            let result = make(1).run_with_team(&loaded, &one, &mut |_| Ok(()));
            if solver == Solver::Cmg && slopes && !g.effects.is_empty() {
                assert_eq!(result.unwrap_err().code, ErrorCode::Unsupported);
                continue;
            }
            let fit = result.unwrap_or_else(|e| panic!("seed {seed}, {solver:?}: {e}"));
            compare(&fit, &g, seed);
            if seed % 20 == 0 {
                let other = make(8)
                    .run_with_team(&loaded, &eight, &mut |_| Ok(()))
                    .unwrap();
                assert_eq!(fit.coefficients, other.coefficients);
                assert_eq!(fit.covariance.values, other.covariance.values);
                assert_eq!(fit.diagnostics, other.diagnostics);
                assert_eq!(fit.outputs, other.outputs);
                assert_eq!(fit.dof, other.dof);
                assert_eq!(fit.statistics, other.statistics);
            }
        }
    }
}
#[test]
fn complete_ols_calls_match_dense_oracles() {
    campaign(0, 200);
}
#[test]
#[ignore = "long complete-controller oracle campaign"]
fn long_complete_ols_calls_match_dense_oracles() {
    campaign(200, 2000);
}
