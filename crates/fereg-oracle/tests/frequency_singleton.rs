#![deny(clippy::too_many_lines)]
#[allow(dead_code)]
#[path = "support/ols_complete_oracle.rs"]
mod dense;
#[allow(dead_code)]
#[path = "support/ols_complete.rs"]
mod fixture;
#[allow(dead_code)]
mod support;
use fereg_core::{
    fit_prepared_model_unadjusted_with_loaded_moments, prepare_categorical_ols_data,
    PreparedOlsAbsorbedDofPolicy, PreparedOlsExecutionMode, PreparedOlsUnadjustedConfig,
};
use fereg_oracle::compare::PREDICTION;

#[test]
fn frequency_retained_singleton_factor_is_absorbed_by_its_group_slope() {
    let case = fixture::case(66);
    let oracle = dense::independent(&case);
    let prepared = prepare_categorical_ols_data(&case.request, &case.loaded).unwrap();
    assert_eq!(prepared.sample().retained_rows(), oracle.rows);
    assert!(oracle.rows.contains(&(case.loaded.candidate.len() - 1)));
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let mut config = PreparedOlsUnadjustedConfig {
            absorbed_dof: PreparedOlsAbsorbedDofPolicy::None,
            ..PreparedOlsUnadjustedConfig::default()
        };
        config.fit.execution.mode = mode;
        config.fit.execution.poolsize = 1;
        let result =
            fit_prepared_model_unadjusted_with_loaded_moments(&prepared, config, None).unwrap();
        assert_eq!(
            result.dof().residual_dof,
            oracle.result.residual_degrees_of_freedom
        );
        assert_eq!(
            result.fit().finite_rank(),
            oracle.result.finite_regressor_rank
        );
        PREDICTION
            .check_slice(result.fit().residuals(), &oracle.result.residuals)
            .unwrap();
        assert!(result.fit().rank_certificate().passed);
        assert!(result.fit().score_certificate().passed);
        assert!(result.certificate().passed);
    }
}
