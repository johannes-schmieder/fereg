//! Bounded optional-builder witnesses, independent of the routing controller.
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;
#[allow(dead_code)]
#[path = "replay/wire.rs"]
mod wire;
#[path = "../tests/support/routing_overlap.rs"]
mod witness;

use fereg_oracle::explicit_ols::fit_explicit_ols;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn main() {
    let directory = PathBuf::from(std::env::args().nth(1).expect("output directory"));
    fs::create_dir_all(&directory).unwrap();
    for weighted in [false, true] {
        write_witness(&directory, weighted, false, witness::witness(weighted));
        write_witness(
            &directory,
            weighted,
            true,
            witness::three_rhs_low_witness(weighted),
        );
    }
}

fn write_witness(directory: &Path, weighted: bool, three_rhs: bool, witness: witness::Witness) {
    let design = &witness.design;
    let (problem, rows) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap();
    let rank = witness.beta.len();
    assert_eq!(oracle.finite_regressor_rank, rank);
    assert_eq!(oracle.explicit_dummy_rank, 2 * witness::LEVELS - 2);
    assert_eq!(rows, (0..8960).collect::<Vec<_>>());
    for (actual, expected) in oracle.coefficients.iter().zip(&witness.beta) {
        assert!((actual.unwrap() - expected).abs() < 1e-11);
    }
    assert!(oracle
        .residuals
        .iter()
        .zip(&witness.residuals)
        .all(|(actual, expected)| (actual - expected).abs() < 1e-10));
    let constant = design
        .weights
        .iter()
        .zip(&oracle.absorbed_fitted_values)
        .map(|(weight, value)| weight * value)
        .sum::<f64>()
        / design.weights.iter().sum::<f64>();
    assert!((constant - witness.constant).abs() < 1e-11);
    let mut beta = witness.beta.clone();
    beta.push(witness.constant);
    let matrix = oracle
        .normalized_reported_intercept_hc1_vce
        .as_ref()
        .unwrap();
    let covariance: Vec<Vec<_>> = (0..=rank)
        .map(|row| (0..=rank).map(|column| matrix[(row, column)]).collect())
        .collect();
    let modes: &[bool] = if three_rhs { &[true] } else { &[false, true] };
    for &low in modes {
        let name = format!(
            "overlap-{}-{}{}",
            if weighted { "weighted" } else { "unit" },
            if three_rhs { "three-rhs-" } else { "" },
            if low { "low" } else { "fast" }
        );
        let mut request = wire::make(design, false, "robust", "auto", low);
        request.replace("poolsize", if three_rhs { "3" } else { "1" });
        request.replace("tolerance", "1e-12");
        request.save_effects(design);
        let case = directory.join(format!("{name}.case"));
        assert!(!case.exists(), "refuse to replace frozen overlap inputs");
        request.write(&case);
        let mut output = fs::File::create(directory.join(format!("{name}.json"))).unwrap();
        writeln!(output, "{{\"schema\":1,\"poisson\":false,\"return_code\":0,\"rows\":{rows:?},\"sample\":{:?},\"beta\":{beta:?},\"covariance\":{covariance:?},\"residual\":{:?},\"fitted\":{:?},\"outputs\":{{", vec![1; rows.len()], witness.residuals, oracle.fitted_values).unwrap();
        for (i, (key, value)) in request.outputs.iter().enumerate() {
            writeln!(output, "{}\"{key}\":{value}", if i == 0 { "" } else { "," }).unwrap();
        }
        // Fixed tariff witness: 32 * W0 is 5,924,864 for one active RHS
        // and 11,958,272 for three; both use the declared 64-by-64 graph.
        let initial_setup_limit = if low && !three_rhs {
            5_924_864
        } else {
            11_958_272
        };
        writeln!(output, "}},\"saved\":{:?},\"overlap_reference\":{{\"initial_rhs\":{},\"initial_setup_limit\":{initial_setup_limit},\"initial_response_certified\":{},\"admission_contract\":\"{}\",\"finite_rank\":{rank},\"absorbed_rank\":{},\"residual_dof\":{},\"weighted_rss\":{},\"saved_channels\":{:?}}}}}", request.saved, if low && !three_rhs { 1 } else { 3 }, three_rhs, if low && !three_rhs { "work_budget_refused" } else { "installed" }, oracle.explicit_dummy_rank, oracle.residual_degrees_of_freedom, oracle.weighted_rss, witness.saved_channels).unwrap();
    }
}
