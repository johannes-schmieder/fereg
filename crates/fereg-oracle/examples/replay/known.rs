//! Preserved scientific witnesses; the intended dense targets are computed now.
use super::{dense, fixture, wire};
use fereg_oracle::{
    designs::{Design, WeightKind},
    explicit_ols::DummyEffect,
};
use std::{fs::File, io::Write, path::Path};

pub fn generate(directory: &Path) {
    for seed in [12, 66, 294] {
        let case = fixture::case(seed);
        let oracle = dense::independent(&case);
        let n = case.loaded.candidate.len();
        let shape = Design {
            outcome: vec![0.; n],
            regressors: vec![vec![0.; n]; case.request.model.terms.len()],
            effects: case
                .request
                .absorbed_terms
                .iter()
                .map(|e| {
                    DummyEffect::new(
                        vec![0; n],
                        1,
                        e.intercept,
                        vec![vec![0.; n]; e.slope_sources.len()],
                    )
                    .unwrap()
                })
                .collect(),
            clusters: vec![vec![0; n]; case.clusters.len()],
            weights: vec![1.; n],
            weight_kind: WeightKind::Unit,
            missing: vec![false; n],
        };
        let vce = ["iid", "robust", "cluster", "cluster"][case.vce];
        let input = wire::model_input(&case.request, &case.loaded, &case.clusters, false);
        let mut request = wire::configure(input, &shape, false, vce, "auto", false);
        for (key, value) in [
            (
                "dof",
                ["none", "firstpair", "pairwise", "continuous"][case.dof].into(),
            ),
            (
                "include_intercept",
                usize::from(
                    case.request.include_intercept
                        && (case.request.absorbed_terms.is_empty()
                            || case
                                .request
                                .absorbed_terms
                                .iter()
                                .any(|effect| effect.intercept)),
                )
                .to_string(),
            ),
            (
                "keep_singletons",
                usize::from(case.request.keep_singletons).to_string(),
            ),
            ("tolerance", "1e-12".into()),
        ] {
            request.replace(key, &value);
        }
        let name = format!("known-{seed}");
        request.write(&directory.join(format!("{name}.case")));
        let mut file = File::create(directory.join(format!("{name}.json"))).unwrap();
        let (code, message, defect) = match seed {
            12 => (0, "", "full-matrix-covariance"),
            66 => (
                459,
                "prepared OLS finite-score regressor scale is not finite and positive",
                "frequency-singleton",
            ),
            _ => (
                498,
                "categorical absorbed-DoF slope row is inconsistent",
                "slope-dof",
            ),
        };
        let rows = oracle.rows;
        let sample = (0..n)
            .map(|i| usize::from(rows.contains(&i)))
            .collect::<Vec<_>>();
        let beta = oracle
            .result
            .coefficients
            .iter()
            .map(|v| v.unwrap_or(0.))
            .collect::<Vec<_>>();
        let slots = oracle.slots;
        let fitted = oracle.result.fitted_values;
        let residual = oracle.result.residuals;
        // M0 records both the specific observed defect and the independent result;
        // a known error never counts as a scientific success.
        writeln!(file,"{{\"schema\":1,\"poisson\":false,\"return_code\":{code},\"known_defect\":\"{defect}\",\"error_contains\":\"{message}\",\"rows\":{rows:?},\"sample\":{sample:?},\"beta\":{beta:?},\"beta_indices\":{slots:?},\"residual\":{residual:?},\"fitted\":{fitted:?},\"outputs\":{{").unwrap();
        for (i, (key, value)) in request.outputs.iter().enumerate() {
            writeln!(file, "{}\"{key}\":{value}", if i == 0 { "" } else { "," }).unwrap();
        }
        writeln!(
            file,
            "}},\"covariance\":[],\"independent_raw_covariance\":["
        )
        .unwrap();
        let raw = if seed == 12 {
            oracle
                .result
                .normalized_reported_intercept_clustered_vce
                .as_ref()
                .unwrap()
        } else if seed == 66 {
            &oracle.result.iid_vce
        } else {
            &oracle.result.hc1_vce
        };
        for i in 0..raw.nrows() {
            let row = (0..raw.ncols()).map(|j| raw[(i, j)]).collect::<Vec<_>>();
            writeln!(file, "{}{row:?}", if i == 0 { "" } else { "," }).unwrap();
        }
        let mut covariance_indices = oracle
            .result
            .kept_regressors
            .iter()
            .map(|&i| slots[i])
            .collect::<Vec<_>>();
        if seed == 12 {
            covariance_indices.push(case.request.model.terms.len());
        }
        writeln!(file,"],\"covariance_indices\":{covariance_indices:?},\"independent_residual_dof\":{},\"independent_effective_n\":{}}}",oracle.result.residual_degrees_of_freedom,oracle.result.effective_observations).unwrap();
    }
}
