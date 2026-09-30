//! Independent full-dummy Newton roots, tuple meats and spectral positive parts.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    factor::CategoricalCodeView,
    ppml::{
        inference::{self, Inference},
        irls,
    },
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    vce::Kind,
    wls::{Role, Status},
};
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph},
    explicit_ols::DummyEffect,
    explicit_ppml::{fit_explicit_ppml, PpmlProblem, PpmlResult, PpmlWeights},
};
#[path = "support/engine.rs"]
mod inputs;
#[path = "support/ppml_inference.rs"]
mod pinned;
struct Fixture {
    sample: Prepared,
    columns: CompiledColumns,
    explicit: PpmlProblem,
    clusters: Vec<Vec<u32>>,
    supports_graph: bool,
}
fn compiled(source: usize, normalized: bool) -> CompiledColumns {
    CompiledColumns {
        slots: vec![
            ColumnSlot {
                name: "base".into(),
                syntax: SyntaxStatus::Base,
                generated: None,
            },
            ColumnSlot {
                name: "x".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(0),
            },
            ColumnSlot {
                name: "duplicate".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(1),
            },
            ColumnSlot {
                name: "omitted".into(),
                syntax: SyntaxStatus::ExplicitlyOmitted,
                generated: None,
            },
            ColumnSlot {
                name: "_cons".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(2),
            },
        ],
        recipes: vec![
            ColumnRecipe::Continuous { source },
            ColumnRecipe::Continuous { source },
            ColumnRecipe::Intercept,
        ],
        roles: vec![
            Role::Ordinary,
            Role::Ordinary,
            if normalized {
                Role::AbsorbedIntercept
            } else {
                Role::Ordinary
            },
        ],
        constant_slot: Some(4),
        structural_intercept: true,
        implicit_constant: false,
    }
}
fn fixture(seed: usize) -> Fixture {
    let graphs = [
        Graph::DensePanel,
        Graph::AkmStrong,
        Graph::AkmWeak,
        Graph::Chain,
        Graph::Barbell,
        Graph::Disconnected,
    ];
    let slope_only = seed % 13 == 0;
    let slopes = slope_only || seed % 5 == 0;
    let g = generate(
        seed as u64,
        &DesignSpec {
            rows: 79 + seed % 31,
            levels: vec![6, 5, 3][..seed % 4].to_vec(),
            regressors: 1,
            slopes,
            graph: graphs[(seed / 4) % 6],
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let effects: Vec<_> = g
        .effects
        .into_iter()
        .map(|e| {
            DummyEffect::new(
                e.codes().to_vec(),
                e.n_levels(),
                !slope_only,
                e.slopes().to_vec(),
            )
            .unwrap()
        })
        .collect();
    let n = g.outcome.len();
    let y: Vec<_> = (0..n)
        .map(|i| 0.5 + ((i * 17 + seed * 3) % 11) as f64 * 0.15)
        .collect();
    let mut sample = inputs::prepare(&effects, &y);
    let source = sample.regressors.len();
    sample.regressors.push(g.regressors[0].clone());
    let normalized = effects.iter().any(|e| e.has_intercept());
    let columns = compiled(source, normalized);
    let values: Vec<_> = (0..n).map(|i| (1 + (i + seed) % 5) as f64).collect();
    let weights = match seed % 3 {
        0 => PpmlWeights::Unit,
        1 => PpmlWeights::Frequency(values.iter().map(|v| *v as u32).collect()),
        _ => PpmlWeights::Probability(values.clone()),
    };
    if seed % 3 != 0 {
        sample.weights = PreparedWeights::Provided {
            kind: if seed % 3 == 1 {
                WeightKind::Frequency
            } else {
                WeightKind::Probability
            },
            values,
        };
    }
    let offset: Vec<_> = (0..n)
        .map(|i| ((i + seed) % 7) as f64 * 0.06 - 0.18)
        .collect();
    sample.adjustment = match (seed / 3) % 3 {
        0 => PreparedAdjustment::None,
        1 => PreparedAdjustment::Offset(offset.clone()),
        _ => PreparedAdjustment::Exposure(offset.iter().map(|v| v.exp()).collect()),
    };
    let dimensions = (seed / 3) % 6;
    let mut clusters: Vec<Vec<u32>> = (0..dimensions)
        .map(|j| {
            (0..n)
                .map(|i| ((i / (j + 2) + seed * (j + 1)) % (j + 3)) as u32)
                .collect()
        })
        .collect();
    if seed % 17 == 0 && dimensions > 0 && normalized {
        clusters[0] = effects[0].codes().to_vec();
    }
    let mut explicit_columns = vec![g.regressors[0].clone()];
    if !normalized {
        explicit_columns.push(vec![1.; n]);
    }
    for effect in &effects {
        for level in 0..effect.n_levels() {
            for channel in effect
                .has_intercept()
                .then(|| vec![1.; n])
                .into_iter()
                .chain(effect.slopes().iter().cloned())
            {
                explicit_columns.push(
                    (0..n)
                        .map(|i| {
                            if effect.codes()[i] as usize == level {
                                channel[i]
                            } else {
                                0.
                            }
                        })
                        .collect(),
                );
            }
        }
    }
    let explicit = PpmlProblem {
        design: (0..n)
            .map(|i| explicit_columns.iter().map(|x| x[i]).collect())
            .collect(),
        outcome: y,
        offset: match &sample.adjustment {
            PreparedAdjustment::None => vec![],
            PreparedAdjustment::Offset(v) => v.clone(),
            PreparedAdjustment::Exposure(v) => v.iter().map(|v| v.ln()).collect(),
        },
        weights,
        clusters: clusters
            .iter()
            .map(|c| c.iter().map(|&v| u64::from(v)).collect())
            .collect(),
    };
    Fixture {
        sample,
        columns,
        explicit,
        clusters,
        supports_graph: effects.len() >= 2 && !slopes,
    }
}
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "seed={seed}: {a:.17e} != {b:.17e}"
    );
}
// Independent closed-form spectral positive part for the one/two identified
// finite coordinates. The full oracle sandwich is formed before extracting it.
fn positive_part(matrix: &[Vec<f64>], k: usize) -> Vec<f64> {
    if k == 1 {
        return vec![matrix[0][0].max(0.)];
    }
    let (a, b, d) = (matrix[0][0], matrix[0][1], matrix[1][1]);
    let center = 0.5 * a + 0.5 * d;
    let radius = (0.5 * a - 0.5 * d).hypot(b);
    let (lo, hi) = (center - radius, center + radius);
    if lo >= 0. {
        return vec![a, b, b, d];
    }
    if hi <= 0. {
        return vec![0.; 4];
    }
    let factor = hi / (2. * radius);
    vec![factor * (a - lo), factor * b, factor * b, factor * (d - lo)]
}
fn compare(actual: &Inference, fit: &irls::Fit<'_>, expected: &PpmlResult, seed: usize) {
    close(actual.coefficients[1], expected.coefficients[0], seed);
    let normalized = actual.statuses[4] == Status::NormalizedAbsorbedIntercept;
    assert_eq!(
        actual.statuses[..4],
        [
            Status::Base,
            Status::Kept,
            Status::NumericallyRankDeficient,
            Status::ExplicitlyOmitted
        ]
    );
    let coordinates = if normalized { vec![1] } else { vec![1, 4] };
    assert_eq!(actual.coordinates, coordinates);
    if !normalized {
        close(actual.coefficients[4], expected.coefficients[1], seed);
    }
    let k = coordinates.len();
    let oracle = positive_part(&expected.covariance, k);
    for (i, &slot) in coordinates.iter().enumerate() {
        for (j, &other) in coordinates.iter().enumerate() {
            close(
                actual.covariance.values[(slot, other)],
                oracle[i * k + j],
                seed,
            );
            close(
                fit.linear.finite.bread[(if i == 0 { 0 } else { 2 }, if j == 0 { 0 } else { 2 })],
                expected.bread[i][j],
                seed,
            );
        }
    }
    for slot in [0, 2, 3].into_iter().chain(normalized.then_some(4)) {
        assert_eq!(actual.coefficients[slot], 0.);
        assert!(actual.covariance.standard_errors[slot].is_nan());
        assert!(actual.z_statistics[slot].is_nan());
        for j in 0..5 {
            assert_eq!(actual.covariance.values[(slot, j)], 0.);
        }
    }
    if oracle[0] > 0. {
        close(
            actual.wald.statistic.unwrap(),
            expected.coefficients[0].powi(2) / oracle[0],
            seed,
        );
        close(
            actual.z_statistics[1],
            expected.coefficients[0] / oracle[0].sqrt(),
            seed,
        );
    } else {
        assert_eq!(actual.wald.statistic, None);
        assert!(actual.z_statistics[1].is_nan());
    }
    for (a, b) in fit.point.eta.iter().zip(&expected.eta) {
        close(*a, *b, seed);
    }
    for (a, b) in fit.evaluation.mean.iter().zip(&expected.mean) {
        close(*a, *b, seed);
    }
    close(fit.evaluation.objective.deviance, expected.deviance, seed);
}
fn identical(a: &Inference, b: &Inference) {
    let bits = |v: &[f64]| v.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&a.coefficients), bits(&b.coefficients));
    assert_eq!(
        bits(a.covariance.values.as_slice()),
        bits(b.covariance.values.as_slice())
    );
    assert_eq!(
        bits(&a.covariance.standard_errors),
        bits(&b.covariance.standard_errors)
    );
    assert_eq!(bits(&a.z_statistics), bits(&b.z_statistics));
    assert_eq!(a.wald, b.wald);
    assert_eq!(a.dof, b.dof);
    assert_eq!(a.absorbed_dof, b.absorbed_dof);
    assert_eq!(a.coordinates, b.coordinates);
    assert_eq!(a.statuses, b.statuses);
    assert_eq!(a.covariance.psd, b.covariance.psd);
    assert_eq!(
        a.covariance.correction.to_bits(),
        b.covariance.correction.to_bits()
    );
    assert_eq!(
        a.covariance.reconstruction_error.to_bits(),
        b.covariance.reconstruction_error.to_bits()
    );
    assert_eq!(a.covariance.cluster_counts, b.covariance.cluster_counts);
    assert_eq!(a.covariance.subset_counts, b.covariance.subset_counts);
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    let mut repaired = 0;
    for seed in start..start + count {
        let f = fixture(seed);
        let raw = f.columns.generate(&f.sample).unwrap();
        let views: Vec<_> = f
            .clusters
            .iter()
            .map(|c| CategoricalCodeView::new(c, 1 + *c.iter().max().unwrap() as usize))
            .collect();
        let expected =
            fit_explicit_ppml(&f.explicit).unwrap_or_else(|e| panic!("oracle seed={seed}: {e}"));
        let mut solvers = vec![Solver::Auto];
        if f.supports_graph {
            solvers.extend([Solver::Lsmr, Solver::Cmg]);
        }
        let problem = irls::Problem {
            sample: &f.sample,
            raw: &raw,
            roles: &f.columns.roles,
        };
        for solver in solvers {
            let options = Options {
                solver,
                ..Options::for_estimator(Estimator::Ppml)
            };
            let fit = irls::fit_final_sample(&problem, options, &one)
                .unwrap_or_else(|e| panic!("root seed={seed}/{solver:?}: {e}"));
            let kind = if views.is_empty() {
                Kind::Robust
            } else {
                Kind::Cluster
            };
            let actual = inference::estimate(&f.columns, &fit, &views, kind, 1e-8, &one)
                .unwrap_or_else(|e| panic!("inference seed={seed}/{solver:?}: {e}"));
            compare(&actual, &fit, &expected, seed);
            pinned::compare(&f.sample, &fit, &actual, &f.clusters, seed);
            repaired += usize::from(actual.covariance.psd.adjusted);
            if seed % 41 == 0 {
                let repeated = irls::fit_final_sample(&problem, options, &eight).unwrap();
                assert_eq!(fit, repeated);
                identical(
                    &actual,
                    &inference::estimate(&f.columns, &repeated, &views, kind, 1e-8, &eight)
                        .unwrap(),
                );
            }
        }
    }
    assert!(repaired > 0, "campaign must exercise multiway PSD repair");
}
#[test]
fn accepted_inference_matches_dense_newton_and_pinned_legacy() {
    campaign(0, 200);
}
#[test]
#[ignore = "long PPML accepted-inference oracle and pinned differential campaign"]
fn two_thousand_accepted_inference_designs() {
    campaign(200, 2000);
}
