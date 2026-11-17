use super::*;

#[test]
fn opposite_extreme_values_with_tiny_positive_weights_preserve_one_fe_span() {
    // Independent weighted normal equations: sum(w*x)=sum(w*y)=0,
    // sum(w*x*y)=.02, sum(w*x*x)=2e306, so beta=1e-308.
    // All input values, the weighted Gram, inverse, and fitted values are
    // finite. The difference between the first two x values overflows.
    let x = vec![1e308, -1e308, 0., 0.];
    let y = [1., -1., -0.25, 0.25];
    let w = [1e-310, 1e-310, 1., 1.];
    let g = [1.; 4];
    let data = prepared(
        &y,
        &[x],
        &[false],
        Some(&g),
        true,
        TestWeights::Analytic(&w),
    );
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let config = PreparedOlsFitConfig {
            execution: PreparedOlsExecutionConfig {
                mode,
                poolsize: 1,
                ..PreparedOlsExecutionConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        };
        let result = fit_prepared_model_coefficients(&data, config)
            .unwrap_or_else(|error| panic!("mode={mode:?}: {error:?}"));
        let coefficient = result
            .coefficients()
            .iter()
            .find(|c| c.generated_column_index == Some(0))
            .unwrap()
            .value
            .unwrap();
        assert!((coefficient * 1e308 - 1.).abs() < 1e-12);
        let bread = result.bread().values()[0];
        assert!((bread * 1e307 - 2.5).abs() < 1e-11);
        let iid = crate::ols::fit_prepared_model_unadjusted_with_loaded_moments(
            &data,
            crate::ols::PreparedOlsUnadjustedConfig {
                fit: config,
                ..Default::default()
            },
            None,
        )
        .unwrap();
        assert!(
            (iid.covariance().standard_errors()[0] / 1.767766952966369e-154 - 1.).abs() < 1e-12
        );
        for (&actual, expected) in result.residuals().iter().zip([0., 0., -0.25, 0.25]) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }
    let source = std::sync::Arc::new(data);
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let config = PreparedOlsFitConfig {
            absorb: AbsorbConfig {
                solver: FeSolverKind::Map,
                ..AbsorbConfig::default()
            },
            execution: PreparedOlsExecutionConfig {
                mode,
                poolsize: 1,
                ..PreparedOlsExecutionConfig::default()
            },
            ..PreparedOlsFitConfig::default()
        };
        let prepared = source.as_ref();
        let frame = LinearSolveFrame::from_prepared_ols(prepared).unwrap();
        let mut session = CertifiedLinearSession::new_with_fe_design(
            prepared,
            frame,
            config,
            LinearSessionGenerations::new(0, 0, 0, 0, 0),
            prepared.fit_owned_fe_design().unwrap(),
        )
        .unwrap();
        session
            .enable_group_centered_one_fe(source.clone(), u128::MAX)
            .unwrap();
        let output = session
            .solve_fresh()
            .unwrap_or_else(|error| panic!("implicit mode={mode:?}: {error:?}"));
        let coefficient = output
            .coefficients()
            .find(|c| c.generated_column_index == Some(0))
            .unwrap()
            .value
            .unwrap();
        assert!((coefficient * 1e308 - 1.).abs() < 1e-12);
    }
}
