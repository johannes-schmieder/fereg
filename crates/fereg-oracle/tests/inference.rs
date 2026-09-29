#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::{DenseMatrix, PreparedWeights, ReferenceWeights, WeightKind as EngineWeight},
    dof::ResidualDof,
    factor::CategoricalCodeView,
    spec::Estimator,
    threads::ThreadTeam,
    vce::{self, Kind, Problem},
    wls,
};
use fereg_oracle::{
    designs::{generate, DesignSpec, WeightKind},
    explicit_ols::{fit_explicit_ols, ExplicitOlsProblem, OracleMatrix, OracleWeights},
};
fn compare(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() < 2e-9 * a.abs().max(b.abs()).max(1.),
        "seed {seed}: {a:.17e} != {b:.17e}"
    );
}
// Closed-form spectral positive part in dimension <=2, independent of the
// production Jacobi rotations, pivot cache and reconstruction certificate.
fn positive_part(matrix: &OracleMatrix) -> Vec<f64> {
    if matrix.nrows() == 1 {
        return vec![matrix[(0, 0)].max(0.)];
    }
    let (a, b, d) = (matrix[(0, 0)], matrix[(0, 1)], matrix[(1, 1)]);
    let center = 0.5 * a + 0.5 * d;
    let radius = (0.5 * a - 0.5 * d).hypot(b);
    let lo = center - radius;
    let hi = center + radius;
    if lo >= 0. {
        return matrix.as_slice().to_vec();
    }
    if hi <= 0. {
        return vec![0.; 4];
    }
    let factor = hi / (2. * radius);
    vec![factor * (a - lo), factor * b, factor * b, factor * (d - lo)]
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    let mut repaired = 0;
    for seed in start..start + count {
        let weight_kind = [
            WeightKind::Unit,
            WeightKind::Analytic,
            WeightKind::Frequency,
            WeightKind::Probability,
        ][seed % 4];
        let g = generate(
            seed as u64,
            &DesignSpec {
                rows: 31 + seed % 61,
                levels: vec![],
                regressors: 1 + seed % 2,
                weights: weight_kind,
                clusters: 1 + seed % 3,
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let raw = match weight_kind {
            WeightKind::Unit => PreparedWeights::None,
            _ => PreparedWeights::Provided {
                kind: match weight_kind {
                    WeightKind::Analytic => EngineWeight::Analytic,
                    WeightKind::Frequency => EngineWeight::Frequency,
                    _ => EngineWeight::Probability,
                },
                values: g.weights.clone(),
            },
        };
        let weights = ReferenceWeights::new(&raw, g.outcome.len(), Estimator::Ols).unwrap();
        let w = (0..g.outcome.len())
            .map(|i| weights.value(i))
            .collect::<Vec<_>>();
        let expected = fit_explicit_ols(&ExplicitOlsProblem {
            outcome: g.outcome.clone(),
            regressors: g.regressors.clone(),
            effects: vec![],
            weights: if weight_kind == WeightKind::Frequency {
                OracleWeights::Frequency(g.weights.iter().map(|w| *w as u32).collect())
            } else {
                OracleWeights::Wls(w.clone())
            },
            clusters: g.clusters.clone(),
        })
        .unwrap();
        let design = DenseMatrix::from_columns(&g.regressors).unwrap();
        let slots = (0..design.ncols())
            .map(|j| wls::Slot {
                role: wls::Role::Ordinary,
                reference_norm: (0..design.nrows())
                    .fold(0_f64, |a, i| a.hypot(w[i].sqrt() * design[(i, j)])),
            })
            .collect::<Vec<_>>();
        let fit = wls::solve(
            &wls::Problem {
                design: &design,
                outcome: &g.outcome,
                weights: &w,
                slots: &slots,
                has_absorbed_intercept: false,
            },
            wls::Policy::default(),
            &one,
        )
        .unwrap();
        let cluster_codes = g
            .clusters
            .iter()
            .map(|codes| codes.iter().map(|c| *c as u32).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let clusters = cluster_codes
            .iter()
            .map(|c| CategoricalCodeView::new(c, 1 + *c.iter().max().unwrap() as usize))
            .collect::<Vec<_>>();
        for kind in [Kind::Iid, Kind::Robust, Kind::Cluster] {
            if kind == Kind::Iid && weight_kind == WeightKind::Probability {
                continue;
            }
            let counts = if kind == Kind::Cluster {
                expected.cluster_counts.as_slice()
            } else {
                &[]
            };
            let dof = ResidualDof::new(
                Estimator::Ols,
                weights.effective(),
                fit.kept.len(),
                0,
                0,
                counts,
            )
            .unwrap();
            let p = Problem {
                design: &design,
                bread: &fit.bread,
                residual: &fit.residuals,
                weights,
                clusters: if kind == Kind::Cluster {
                    &clusters
                } else {
                    &[]
                },
                dof,
            };
            let actual = vce::estimate(&p, kind, 5e-10, &one)
                .unwrap_or_else(|e| panic!("seed {seed}/{kind:?}: {e}"));
            if seed % 10 == 0 {
                assert_eq!(
                    actual,
                    vce::estimate(&p, kind, 5e-10, &eight).unwrap(),
                    "seed {seed}: workers"
                );
            }
            let oracle = match kind {
                Kind::Iid => expected.iid_vce.as_slice().to_vec(),
                Kind::Robust => expected.hc1_vce.as_slice().to_vec(),
                Kind::Cluster => positive_part(expected.clustered_vce.as_ref().unwrap()),
            };
            for (a, b) in actual.values.as_slice().iter().zip(oracle) {
                compare(*a, b, seed);
            }
            repaired += usize::from(actual.psd.adjusted);
            if kind == Kind::Cluster {
                assert_eq!(actual.cluster_counts, expected.cluster_counts);
                assert_eq!(actual.subset_counts.len(), (1 << clusters.len()) - 1);
            }
        }
    }
    assert!(repaired > 0, "campaign must exercise multiway PSD repair");
}
#[test]
fn covariance_matches_dense_oracle_and_closed_form_psd() {
    campaign(0, 200);
}
#[test]
#[ignore = "long inference differential campaign"]
fn two_thousand_inference_designs() {
    campaign(200, 2000);
}
