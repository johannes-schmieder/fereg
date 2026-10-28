//! A compact design wider than the 64-column stack bindings: 67 regressors
//! and an absorbed intercept give 68 generated and 69 raw columns, so the
//! wide raw-moment and finite-moment paths are exercised under 1 MB.
use super::{expected, wire};
use fereg_oracle::designs::{generate, DesignSpec};
use std::path::Path;

pub fn generate_case(directory: &Path) {
    let design = generate(
        6_567,
        &DesignSpec {
            rows: 500,
            levels: vec![6, 5],
            regressors: 67,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let request = wire::make(&design, false, "iid", "auto", false);
    let name = "ols-wide-k67";
    request.write(&directory.join(format!("{name}.case")));
    expected::write(
        &directory.join(format!("{name}.json")),
        &design,
        false,
        "iid",
        &request,
    );
}
