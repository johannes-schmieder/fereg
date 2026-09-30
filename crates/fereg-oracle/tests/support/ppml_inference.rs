//! The frozen reference is only a differential check, never the oracle.
use fereg_engine::{
    data::{Prepared, ReferenceWeights, WeightKind},
    ppml::{inference::Inference, irls},
    spec::Estimator,
};
fn close(a: f64, b: f64, seed: usize) {
    if a.is_nan() || b.is_nan() {
        assert!(
            a.is_nan() && b.is_nan(),
            "seed={seed}: missing statistic differs"
        );
    } else {
        assert!(
            (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
            "seed={seed}: {a} != {b}"
        );
    }
}
pub fn compare(
    sample: &Prepared,
    fit: &irls::Fit<'_>,
    actual: &Inference,
    clusters: &[Vec<u32>],
    seed: usize,
) {
    let q = ReferenceWeights::new(&sample.weights, sample.n_active(), Estimator::Ppml).unwrap();
    let k = fit.linear.finite.kept.len();
    let mut design = fereg_core::DenseMatrix::zeros(sample.n_active(), k).unwrap();
    let mut bread = vec![0.; k * k];
    for (i, &j) in fit.linear.finite.kept.iter().enumerate() {
        for row in 0..sample.n_active() {
            design[(row, i)] = fit.linear.within[(row, j)];
        }
        for (m, &other) in fit.linear.finite.kept.iter().enumerate() {
            bread[i * k + m] = fit.linear.finite.bread[(j, other)];
        }
    }
    let prior: Vec<_> = (0..sample.n_active()).map(|i| q.value(i)).collect();
    let residual: Vec<_> = sample
        .outcome
        .iter()
        .zip(&fit.evaluation.mean)
        .map(|(y, mu)| y - mu)
        .collect();
    let cluster_slices: Vec<_> = clusters.iter().map(Vec::as_slice).collect();
    let report = fereg_core::ppml::PreparedPpmlWeightReport {
        kind: q.kind().map(|w| match w {
            WeightKind::Frequency => fereg_core::WeightKind::Frequency,
            WeightKind::Probability => fereg_core::WeightKind::Probability,
            _ => unreachable!(),
        }),
        physical_observations: sample.n_active(),
        effective_observations: q.effective(),
        raw_weight_sum: q.raw_sum,
        log_raw_weight_sum: q.log_raw_sum,
        prepared_weight_sum: q.effective() as f64,
        raw_to_prepared_multiplier: Some(q.log_multiplier().exp()),
        log_raw_to_prepared_multiplier: q.log_multiplier(),
    };
    let expected = fereg_core::ppml::compute_ppml_inference(fereg_core::ppml::PpmlInferenceInput {
        coefficients: &actual.coefficients,
        full_dimension: actual.coefficients.len(),
        within_design: &design,
        reported_finite_rank: actual.dof.finite,
        score_residuals: &residual,
        prior_weights: &prior,
        weight_report: report,
        identified_bread: &bread,
        kept_full_slots: &actual.coordinates,
        absorbed_rank: actual.dof.absorbed,
        connected_components: sample.topology.as_ref().unwrap().components().n_components,
        absorbed_rank_is_approximate: actual.absorbed_dof.is_approximate,
        clusters: &cluster_slices,
        requested_vce: if clusters.is_empty() {
            fereg_core::ppml::PpmlVceRequest::Robust
        } else {
            fereg_core::ppml::PpmlVceRequest::Clustered
        },
        wald_columns: &actual.wald.tested,
        certificate_tolerance: 1e-8,
    })
    .unwrap_or_else(|e| panic!("pinned seed={seed}: {e}"));
    assert_eq!(expected.dof.effective_observations, actual.dof.effective);
    assert_eq!(expected.dof.finite_within_rank, actual.dof.finite);
    assert_eq!(expected.dof.structural_residual, actual.dof.structural);
    assert_eq!(expected.dof.reported, actual.dof.reported);
    assert_eq!(
        expected.vce.cluster_dimension_groups,
        actual.covariance.cluster_counts
    );
    assert_eq!(
        expected.vce.cluster_subsets.len(),
        actual.covariance.subset_counts.len()
    );
    for subset in &expected.vce.cluster_subsets {
        assert_eq!(
            subset.groups,
            actual.covariance.subset_counts[usize::from(subset.mask) - 1]
        );
    }
    close(expected.vce.correction, actual.covariance.correction, seed);
    for (a, b) in expected
        .vce
        .covariance
        .iter()
        .zip(actual.covariance.values.as_slice())
    {
        close(*a, *b, seed);
    }
    for &slot in &actual.coordinates {
        close(
            expected.vce.standard_errors[slot],
            actual.covariance.standard_errors[slot],
            seed,
        );
        close(
            expected.vce.z_statistics[slot],
            actual.z_statistics[slot],
            seed,
        );
    }
    assert_eq!(
        expected.wald.degrees_of_freedom,
        actual.wald.degrees_of_freedom
    );
    match (expected.wald.statistic, actual.wald.statistic) {
        (Some(a), Some(b)) => close(a, b, seed),
        (None, None) => (),
        _ => panic!("seed={seed}: Wald presence differs"),
    }
}
