//! Four additive positive-direct/refusal cases; leaves the 73-case replay alone.
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;
#[allow(dead_code)]
#[path = "replay/wire.rs"]
mod wire;
#[path = "../tests/support/routing_direct.rs"]
mod witness;

use fereg_oracle::explicit_ols::fit_explicit_ols;
use std::{fs, io::Write, path::PathBuf};

fn main() {
    let directory = PathBuf::from(std::env::args().nth(1).expect("output directory"));
    fs::create_dir_all(&directory).unwrap();
    for weighted in [false, true] {
        let design = witness::design(weighted);
        let (problem, rows) = design.oracle_problem(false).unwrap();
        let oracle = fit_explicit_ols(&problem).unwrap();
        assert_eq!(oracle.finite_regressor_rank, 63);
        assert_eq!(rows.len(), 128);
        let mut beta: Vec<_> = oracle.coefficients.iter().map(|b| b.unwrap()).collect();
        beta.push(witness::reported_constant(&design, &oracle));
        // Fast absorption packs y, every control, and the generated intercept.
        // The reported coefficient/covariance dimension excludes y.
        let reported_columns = beta.len();
        let rhs = reported_columns + 1;
        let matrix = oracle
            .normalized_reported_intercept_hc1_vce
            .as_ref()
            .unwrap();
        let covariance: Vec<Vec<_>> = (0..64)
            .map(|r| (0..64).map(|c| matrix[(r, c)]).collect())
            .collect();
        let channels = witness::saved_channels(&design, &oracle);
        for low in [false, true] {
            let name = format!(
                "direct-{}-{}",
                if weighted { "weighted" } else { "unit" },
                if low { "low" } else { "fast" }
            );
            let mut request = wire::make(&design, false, "robust", "auto", low);
            request.replace("poolsize", "63");
            request.replace("tolerance", "1e-12");
            request.save_effects(&design);
            let case = directory.join(format!("{name}.case"));
            assert!(!case.exists(), "refuse to replace frozen direct inputs");
            request.write(&case);
            let mut output = fs::File::create(directory.join(format!("{name}.json"))).unwrap();
            writeln!(output, "{{\"schema\":1,\"poisson\":false,\"return_code\":0,\"rows\":{rows:?},\"sample\":{:?},\"beta\":{beta:?},\"covariance\":{covariance:?},\"residual\":{:?},\"fitted\":{:?},\"outputs\":{{", vec![1;128], oracle.residuals, oracle.fitted_values).unwrap();
            for (i, (key, value)) in request.outputs.iter().enumerate() {
                writeln!(output, "{}\"{key}\":{value}", if i == 0 { "" } else { "," }).unwrap();
            }
            writeln!(output, "}},\"saved\":{:?},\"direct_reference\":{{\"rhs\":{rhs},\"reported_columns\":{reported_columns},\"outcome_columns\":1,\"require_admission\":{},\"finite_rank\":63,\"absorbed_rank\":{},\"residual_dof\":{},\"weighted_rss\":{},\"saved_channels\":{channels:?}}}}}", request.saved, !low, oracle.explicit_dummy_rank, oracle.residual_degrees_of_freedom, oracle.weighted_rss).unwrap();
        }
    }
}
