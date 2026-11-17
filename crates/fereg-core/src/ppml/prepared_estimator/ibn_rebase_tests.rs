use super::*;

#[test]
fn unbased_factor_keeps_original_axis_after_missingness_and_separation() {
    // The wire carries unbased indicator recipes even when Stata's raw operator
    // has lost its `bn` spelling. A vanished level must not choose a new base.
    for separated in [false, true] {
        let (mut request, mut loaded) = factor_rebase_fixture();
        let g = request.ols.model.sources[0].id;
        request.ols.model.terms[0] = categorical_term(1, g, 1.0, false);
        request.output.mean = true;
        request.output.requested_observation_columns = 1;
        if separated {
            loaded.ols.outcome.missing.fill(false);
            loaded.ols.outcome.values[..2].fill(0.0);
        }
        let result = fit_prepared_ppml(request, loaded).expect("unbased factor fit");
        assert_eq!(result.sample().original_rows(), (2..12).collect::<Vec<_>>());
        let statuses = result.coefficients().statuses();
        assert_eq!(statuses.len(), 5);
        assert_eq!(statuses[0], PpmlCoefficientStatus::NumericallyRankDeficient);
        assert_eq!(statuses[3], PpmlCoefficientStatus::SyntaxOmitted);
        assert!(!statuses.contains(&PpmlCoefficientStatus::BaseLevel));
        assert_eq!(result.coefficients().kept_full_slots().len(), 2);
        let values = result.coefficients().values();
        let covariance = &result.inference().vce.covariance;
        for slot in [0, 3] {
            assert_eq!(values[slot], 0.0);
            for other in 0..5 {
                assert_eq!(covariance[slot * 5 + other], 0.0);
                assert_eq!(covariance[other * 5 + slot], 0.0);
            }
        }
        // Independent saturated-cell score solution: means 9/5 and 22/5.
        // HC1 factor 10/9 gives variances 28/729 and 13/1089 for log means.
        let means: [f64; 2] = [9.0 / 5.0, 22.0 / 5.0];
        let variances = [28.0 / 729.0, 13.0 / 1089.0];
        for a in 0..2 {
            assert!((values[a + 1] + values[4] - means[a].ln()).abs() < 1e-9);
            for b in 0..2 {
                let actual = covariance[(a + 1) * 5 + b + 1]
                    + covariance[(a + 1) * 5 + 4]
                    + covariance[4 * 5 + b + 1]
                    + covariance[4 * 5 + 4];
                let expected = if a == b { variances[a] } else { 0.0 };
                assert!((actual - expected).abs() < 1e-9);
            }
        }
        for (row, &mean) in result.output().mean().iter().enumerate() {
            assert!((mean - means[usize::from(row >= 5)]).abs() < 1e-9);
        }
        assert!(result.certificate().passed);
    }
}
