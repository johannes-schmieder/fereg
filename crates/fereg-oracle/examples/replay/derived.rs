//! Fast OLS fits that request no residual, xb, xbd, d or saved-FE output, the
//! public `fereg y x, absorb(...)` call without prediction options. Every
//! other OLS case requests residual, xbd and absorbed outputs, so only these
//! reach the derived within residual (SPEC §7, §16): its certificate accepts
//! `ỹ - X̃b` without a final projection and the residual solver row reports
//! stop code 5. One, two and three categorical-intercept FEs cover the direct
//! one-FE Auto route, the two-FE categorical owner and the many-FE geometry
//! policy; the dense explicit-dummy targets check coefficients and the
//! residual-based covariance.
use super::{expected, wire};
use fereg_oracle::designs::{generate, DesignSpec, WeightKind};
use std::path::Path;

pub fn generate_cases(directory: &Path) {
    for (name, seed, levels, weights, vce) in [
        ("ols-derived-fe1", 7_101, vec![6], WeightKind::Unit, "iid"),
        (
            "ols-derived-fe2",
            7_102,
            vec![6, 5],
            WeightKind::Analytic,
            "robust",
        ),
        (
            "ols-derived-fe3",
            7_103,
            vec![6, 5, 4],
            WeightKind::Unit,
            "robust",
        ),
    ] {
        let design = generate(
            seed,
            &DesignSpec {
                rows: 150,
                levels,
                weights,
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let mut request = wire::make(&design, false, vce, "auto", false);
        request.without_observation_outputs();
        request.write(&directory.join(format!("{name}.case")));
        expected::write(
            &directory.join(format!("{name}.json")),
            &design,
            false,
            vce,
            &request,
        );
    }
}
