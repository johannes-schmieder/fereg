use fereg_oracle::{
    designs::Design, explicit_ols::fit_explicit_ols, explicit_ppml::fit_explicit_ppml,
};
use std::{fs::File, io::Write, path::Path};

pub fn write(
    path: &Path,
    design: &Design,
    poisson: bool,
    vce: &str,
    request: &super::wire::Request,
) {
    let mut file = File::create(path).unwrap();
    let (rows, beta, covariance, residual, fitted) = if poisson {
        let (problem, rows) = design.poisson_problem(false).unwrap();
        let fit = fit_explicit_ppml(&problem).unwrap();
        let k = design.regressors.len();
        let covariance = (0..k)
            .map(|i| fit.covariance[i][..k].to_vec())
            .collect::<Vec<_>>();
        let residual = problem
            .outcome
            .iter()
            .zip(&fit.mean)
            .map(|(y, mu)| y - mu)
            .collect::<Vec<_>>();
        (
            rows,
            fit.coefficients[..k].to_vec(),
            covariance,
            residual,
            fit.mean,
        )
    } else {
        let (problem, rows) = design.oracle_problem(false).unwrap();
        let fit = fit_explicit_ols(&problem).unwrap();
        let matrix = match vce {
            "iid" => &fit.iid_vce,
            "robust" => &fit.hc1_vce,
            _ => fit.clustered_vce.as_ref().unwrap(),
        };
        let covariance = (0..matrix.nrows())
            .map(|i| {
                (0..matrix.ncols())
                    .map(|j| matrix[(i, j)])
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        (
            rows,
            fit.coefficients.iter().map(|v| v.unwrap_or(0.)).collect(),
            covariance,
            fit.residuals,
            fit.fitted_values,
        )
    };
    let sample = (0..design.outcome.len())
        .map(|i| usize::from(rows.contains(&i)))
        .collect::<Vec<_>>();
    writeln!(file, "{{\"schema\":1,\"poisson\":{poisson},\"return_code\":0,\"rows\":{rows:?},\"sample\":{sample:?},\"beta\":{beta:?},\"covariance\":{covariance:?},\"residual\":{residual:?},\"fitted\":{fitted:?},\"outputs\":{{").unwrap();
    for (i, (name, index)) in request.outputs.iter().enumerate() {
        writeln!(file, "{}\"{name}\":{index}", if i == 0 { "" } else { "," }).unwrap();
    }
    writeln!(file, "}},\"saved\":[").unwrap();
    for (i, index) in request.saved.iter().enumerate() {
        writeln!(file, "{}{index}", if i == 0 { "" } else { "," }).unwrap();
    }
    writeln!(file, "]}}").unwrap();
}
