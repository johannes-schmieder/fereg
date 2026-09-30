//! Complete OLS results, preserving original rows and canonical Stata slots.
#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
#[path = "support/ols_complete_oracle.rs"]
mod dense;
#[path = "support/ols_complete.rs"]
mod fixture;
#[allow(dead_code)]
mod support;
use fereg_core as old;
use fereg_engine::{
    dof::{DofPolicy, ResidualDof},
    ols::{self, Request, Resources},
    outputs::{self, Kind},
    spec::{Estimator, Options, Solver},
    vce,
    wls::Status,
};
use fereg_oracle::compare::{Tolerance, COEFFICIENT, INFERENCE, PREDICTION};
use fixture::{case, current, Case};

struct Previous {
    original_slots: usize,
    rows: Vec<usize>,
    drops: Vec<u8>,
    coefficients: Vec<old::PreparedOlsFittedCoefficient>,
    coordinates: Vec<usize>,
    covariance: Vec<f64>,
    standard_errors: Vec<f64>,
    residuals: Vec<f64>,
    dof: Vec<usize>,
    weights: old::PreparedOlsReferenceWeightReport,
    statistics: old::PreparedOlsFitStatistics,
    clusters: Vec<usize>,
    subsets: Vec<usize>,
    psd: Option<old::PreparedOlsClusteredPsdReport>,
    raw_covariance: Vec<f64>,
    absorbed_counts: [usize; 4],
    approximate: bool,
}
macro_rules! snapshot {
    ($fit:expr, $sample:expr, $dof:expr, $clusters:expr, $subsets:expr, $psd:expr) => {{
        let fit = $fit;
        let sample = $sample;
        Previous {
            original_slots: fit.fit().coefficients().len(),
            rows: sample.original_row_by_active.clone(),
            drops: sample.drop_reasons.iter().map(|r| r.code()).collect(),
            coefficients: fit.fit().coefficients().to_vec(),
            coordinates: fit
                .covariance_coordinates()
                .iter()
                .map(|c| c.coefficient_slot.get() as usize - 1)
                .collect(),
            covariance: fit.covariance().values().to_vec(),
            standard_errors: fit.covariance().standard_errors().to_vec(),
            residuals: fit.fit().residuals().to_vec(),
            dof: $dof,
            weights: fit.weights(),
            statistics: fit.statistics(),
            clusters: $clusters,
            subsets: $subsets,
            psd: $psd,
            raw_covariance: vec![],
            absorbed_counts: [
                fit.absorbed_dof().initial_categories,
                fit.absorbed_dof().redundant_categories,
                fit.absorbed_dof().absorbed_rank,
                fit.absorbed_dof().nested_categories,
            ],
            approximate: fit.absorbed_dof().is_approximate,
        }
    }};
}
fn dof_unclustered(d: old::PreparedOlsResidualDofReport) -> Vec<usize> {
    vec![
        d.finite_model_dof,
        d.absorbed_rank,
        0,
        d.residual_dof,
        d.residual_dof,
        d.residual_dof,
        d.residual_dof,
        0,
    ]
}
fn subset_counts(subsets: &[old::PreparedOlsClusterSubsetCertificate]) -> Vec<usize> {
    let mut counts = vec![0; subsets.len()];
    for subset in subsets {
        counts[subset.mask as usize - 1] = subset.groups;
    }
    counts
}
fn previous(case: &Case) -> old::Result<Previous> {
    let mut config = old::PreparedOlsFitConfig::default();
    config.execution.threads = old::ThreadConfig::from_requested(1, true)?;
    config.absorb.map.tolerance = 1e-12;
    let absorbed_dof = [
        old::PreparedOlsAbsorbedDofPolicy::None,
        old::PreparedOlsAbsorbedDofPolicy::FirstPair,
        old::PreparedOlsAbsorbedDofPolicy::Pairwise,
        old::PreparedOlsAbsorbedDofPolicy::Continuous,
    ][case.dof];
    if case.request.absorbed_terms.is_empty() {
        return previous_plain(case, config, absorbed_dof);
    }
    if case.vce >= 2 {
        let prepared = old::prepare_categorical_ols_clustered_data(
            &old::OlsClusteredRequestV1 {
                ols: case.request.clone(),
                clusters: case.clusters.clone(),
            },
            &case.loaded,
        )?;
        let fit = old::fit_prepared_categorical_ols_clustered_map(
            &prepared,
            old::PreparedOlsClusteredConfig {
                fit: config,
                absorbed_dof,
                ..old::PreparedOlsClusteredConfig::default()
            },
        )?;
        let d = fit.dof();
        let mut snapshot = snapshot!(
            &fit,
            prepared.ols().sample(),
            vec![
                d.finite_model_dof,
                d.absorbed_rank,
                d.nested_absorbed_categories,
                d.unclustered_residual_dof,
                d.reported_residual_dof,
                d.fit_residual_dof,
                d.correction_denominator_dof,
                d.minimum_cluster_count
            ],
            fit.cluster_counts().to_vec(),
            subset_counts(fit.subsets()),
            Some(fit.covariance().psd())
        );
        snapshot.raw_covariance = fit.covariance().raw_values().to_vec();
        Ok(lift(snapshot, prepared.ols().report_plan()))
    } else {
        let prepared = old::prepare_categorical_ols_data(&case.request, &case.loaded)?;
        if case.vce == 0 && case.loaded.probability_weight.is_none() {
            let fit = old::fit_prepared_categorical_ols_unadjusted_map(
                &prepared,
                old::PreparedOlsUnadjustedConfig {
                    fit: config,
                    absorbed_dof,
                    ..old::PreparedOlsUnadjustedConfig::default()
                },
            )?;
            Ok(lift(
                snapshot!(
                    &fit,
                    prepared.sample(),
                    dof_unclustered(fit.dof()),
                    vec![],
                    vec![],
                    None
                ),
                prepared.report_plan(),
            ))
        } else {
            let fit = old::fit_prepared_categorical_ols_robust_map(
                &prepared,
                old::PreparedOlsRobustConfig {
                    fit: config,
                    absorbed_dof,
                    ..old::PreparedOlsRobustConfig::default()
                },
            )?;
            Ok(lift(
                snapshot!(
                    &fit,
                    prepared.sample(),
                    dof_unclustered(fit.dof()),
                    vec![],
                    vec![],
                    None
                ),
                prepared.report_plan(),
            ))
        }
    }
}
fn lift(mut snapshot: Previous, plan: &old::PreparedCategoricalOlsReportPlan) -> Previous {
    snapshot.original_slots = plan.original_coefficient_count();
    for coefficient in &mut snapshot.coefficients {
        coefficient.coefficient_slot = plan.original_slot(coefficient.coefficient_slot).unwrap();
    }
    for coordinate in &mut snapshot.coordinates {
        *coordinate = plan.original_slots()[*coordinate].get() as usize - 1;
    }
    snapshot
}
fn previous_plain(
    case: &Case,
    config: old::PreparedOlsFitConfig,
    absorbed_dof: old::PreparedOlsAbsorbedDofPolicy,
) -> old::Result<Previous> {
    if case.vce >= 2 {
        let prepared = old::prepare_ols_clustered_data(
            &old::OlsClusteredRequestV1 {
                ols: case.request.clone(),
                clusters: case.clusters.clone(),
            },
            &case.loaded,
        )?;
        let fit = old::fit_prepared_ols_clustered_map(
            &prepared,
            old::PreparedOlsClusteredConfig {
                fit: config,
                absorbed_dof,
                ..old::PreparedOlsClusteredConfig::default()
            },
        )?;
        let d = fit.dof();
        let mut snapshot = snapshot!(
            &fit,
            &prepared.ols().sample,
            vec![
                d.finite_model_dof,
                d.absorbed_rank,
                d.nested_absorbed_categories,
                d.unclustered_residual_dof,
                d.reported_residual_dof,
                d.fit_residual_dof,
                d.correction_denominator_dof,
                d.minimum_cluster_count
            ],
            fit.cluster_counts().to_vec(),
            subset_counts(fit.subsets()),
            Some(fit.covariance().psd())
        );
        snapshot.raw_covariance = fit.covariance().raw_values().to_vec();
        Ok(snapshot)
    } else {
        let prepared = old::prepare_ols_data(&case.request, &case.loaded)?;
        if case.vce == 0 && case.loaded.probability_weight.is_none() {
            let fit = old::fit_prepared_ols_unadjusted_map(
                &prepared,
                old::PreparedOlsUnadjustedConfig {
                    fit: config,
                    absorbed_dof,
                    ..old::PreparedOlsUnadjustedConfig::default()
                },
            )?;
            Ok(snapshot!(
                &fit,
                &prepared.sample,
                dof_unclustered(fit.dof()),
                vec![],
                vec![],
                None
            ))
        } else {
            let fit = old::fit_prepared_ols_robust_map(
                &prepared,
                old::PreparedOlsRobustConfig {
                    fit: config,
                    absorbed_dof,
                    ..old::PreparedOlsRobustConfig::default()
                },
            )?;
            Ok(snapshot!(
                &fit,
                &prepared.sample,
                dof_unclustered(fit.dof()),
                vec![],
                vec![],
                None
            ))
        }
    }
}
fn dof_current(d: ResidualDof) -> Vec<usize> {
    vec![
        d.finite,
        d.absorbed,
        d.nested,
        d.structural,
        d.reported,
        d.fit,
        d.correction,
        d.minimum_clusters.unwrap_or(0),
    ]
}
fn status_current(status: Status) -> old::PreparedOlsFitStatus {
    use old::PreparedOlsFitStatus as S;
    match status {
        Status::Kept => S::Kept,
        Status::Base | Status::ExplicitlyOmitted => S::SyntaxOmitted,
        Status::AbsorbedInterceptDeferred => S::AbsorbedInterceptDeferred,
        Status::NormalizedAbsorbedIntercept => S::NormalizedAbsorbedIntercept,
        Status::CollinearWithFixedEffects => S::CollinearWithFixedEffects,
        Status::NumericallyRankDeficient => S::NumericallyRankDeficient,
    }
}
fn check(tolerance: Tolerance, actual: f64, expected: f64, seed: usize, channel: &str) {
    tolerance
        .check(actual, expected)
        .unwrap_or_else(|e| panic!("seed {seed}, {channel}: {e}"));
}
fn optional(actual: Option<f64>, expected: Option<f64>, seed: usize, channel: &str) {
    match (actual, expected) {
        (Some(a), Some(b)) => check(INFERENCE, a, b, seed, channel),
        (a, b) => assert_eq!(a, b, "seed {seed}, {channel}"),
    }
}
fn compare(fit: &ols::OlsFit, expected: &Previous, seed: usize) {
    assert_eq!(
        fit.sample.retained_rows, expected.rows,
        "seed {seed}, sample"
    );
    assert_eq!(
        fit.sample
            .drop_reasons
            .iter()
            .map(|r| r.code())
            .collect::<Vec<_>>(),
        expected.drops,
        "seed {seed}, drops"
    );
    assert_eq!(
        fit.coordinates, expected.coordinates,
        "seed {seed}, coordinates"
    );
    assert_eq!(dof_current(fit.dof), expected.dof, "seed {seed}, DoF");
    assert_eq!(
        [
            fit.absorbed_dof.initial_categories,
            fit.absorbed_dof.redundant_categories,
            fit.absorbed_dof.absorbed_rank,
            fit.absorbed_dof.nested_categories
        ],
        expected.absorbed_counts,
        "seed {seed}, absorbed counts"
    );
    assert_eq!(fit.absorbed_dof.is_approximate, expected.approximate);
    assert_eq!(
        fit.weights.physical_rows,
        expected.weights.physical_observations
    );
    assert_eq!(
        fit.weights.effective_rows,
        expected.weights.effective_observations
    );
    optional(
        fit.weights.raw_sum,
        expected.weights.raw_weight_sum,
        seed,
        "raw weight sum",
    );
    check(
        INFERENCE,
        fit.weights.log_multiplier.exp(),
        expected.weights.reference_weight_multiplier / expected.weights.solver_weight_divisor,
        seed,
        "reference weight multiplier",
    );
    assert_eq!(
        fit.covariance.cluster_counts, expected.clusters,
        "seed {seed}, clusters"
    );
    assert_eq!(
        fit.covariance.subset_counts, expected.subsets,
        "seed {seed}, subsets"
    );
    assert_eq!(fit.coefficients.len(), expected.original_slots);
    for coefficient in &expected.coefficients {
        let slot = coefficient.coefficient_slot.get() as usize - 1;
        assert_eq!(
            status_current(fit.statuses[slot]),
            coefficient.status,
            "seed {seed}, status slot {slot}"
        );
        check(
            COEFFICIENT,
            fit.coefficients[slot],
            coefficient.value.unwrap_or(0.),
            seed,
            "b",
        );
    }
    for slot in 0..expected.original_slots {
        if !expected
            .coefficients
            .iter()
            .any(|c| c.coefficient_slot.get() as usize - 1 == slot)
        {
            assert_eq!(fit.statuses[slot], Status::NumericallyRankDeficient);
            assert_eq!(fit.coefficients[slot], 0.);
            for other in 0..expected.original_slots {
                assert_eq!(fit.covariance.values[(slot, other)], 0.);
                assert_eq!(fit.covariance.values[(other, slot)], 0.);
            }
        }
    }
    for (i, &row) in expected.rows.iter().enumerate() {
        check(
            PREDICTION,
            fit.outputs.get(Kind::Residuals).unwrap()[row],
            expected.residuals[i],
            seed,
            "residual",
        );
    }
    statistics(fit, expected, seed);
}
fn compare_inference(fit: &ols::OlsFit, expected: &Previous, seed: usize) {
    let p = expected.coordinates.len();
    for (i, &left) in expected.coordinates.iter().enumerate() {
        check(
            INFERENCE,
            fit.covariance.standard_errors[left],
            expected.standard_errors[i],
            seed,
            "SE",
        );
        let error = expected.standard_errors[i];
        let expected_t = expected
            .coefficients
            .iter()
            .find(|c| c.coefficient_slot.get() as usize - 1 == left)
            .unwrap()
            .value
            .unwrap_or(0.)
            / error;
        if expected_t.is_finite() {
            check(INFERENCE, fit.t_statistics[left], expected_t, seed, "t");
        } else {
            assert!(fit.t_statistics[left].is_nan());
        }
        for (j, &right) in expected.coordinates.iter().enumerate() {
            check(
                INFERENCE,
                fit.covariance.values[(left, right)],
                expected.covariance[i * p + j],
                seed,
                "V",
            );
        }
    }
    optional(
        fit.wald.statistic,
        expected.statistics.f_statistic,
        seed,
        "F",
    );
}
fn statistics(fit: &ols::OlsFit, expected: &Previous, seed: usize) {
    let a = fit.statistics;
    let b = expected.statistics;
    for (a, b, name) in [
        (a.rss, b.rss, "RSS"),
        (a.tss, b.tss, "TSS"),
        (a.within_tss, b.within_tss, "within TSS"),
        (a.rmse, b.root_mean_squared_error, "RMSE"),
    ] {
        check(INFERENCE, a, b, seed, name);
    }
    for (a, b, name) in [
        (a.r_squared, b.r_squared, "R2"),
        (a.adjusted_r_squared, b.adjusted_r_squared, "adjusted R2"),
        (a.within_r_squared, b.within_r_squared, "within R2"),
        (
            a.adjusted_within_r_squared,
            b.adjusted_within_r_squared,
            "adjusted within R2",
        ),
        (a.log_likelihood, b.gaussian_log_likelihood, "ll"),
        (
            a.null_log_likelihood,
            b.gaussian_null_log_likelihood,
            "ll_null",
        ),
    ] {
        optional(a, b, seed, name);
    }
}
fn compare_independent(fit: &ols::OlsFit, oracle: &dense::Independent, seed: usize) {
    assert_eq!(
        fit.sample.retained_rows, oracle.rows,
        "seed {seed}, independent rows"
    );
    assert_eq!(
        fit.sample
            .drop_reasons
            .iter()
            .map(|r| r.code())
            .collect::<Vec<_>>(),
        oracle.drops,
        "seed {seed}, independent ledger"
    );
    assert_eq!(
        fit.dof.finite, oracle.result.finite_regressor_rank,
        "seed {seed}, finite rank"
    );
    assert_eq!(fit.covariance.cluster_counts, oracle.result.cluster_counts);
    assert_eq!(fit.covariance.subset_counts, oracle.subsets);
    for (column, &slot) in oracle.slots.iter().enumerate() {
        check(
            COEFFICIENT,
            fit.coefficients[slot],
            oracle.result.coefficients[column].unwrap_or(0.),
            seed,
            "independent b",
        );
    }
    if let Some(constant) = oracle.constant {
        check(
            COEFFICIENT,
            *fit.coefficients.last().unwrap(),
            constant,
            seed,
            "independent constant",
        );
    }
    check(
        INFERENCE,
        fit.statistics.rss,
        oracle.result.weighted_rss,
        seed,
        "independent RSS",
    );
    for (i, &row) in oracle.rows.iter().enumerate() {
        check(
            PREDICTION,
            fit.outputs.get(Kind::Residuals).unwrap()[row],
            oracle.result.residuals[i],
            seed,
            "independent residual",
        );
        check(
            PREDICTION,
            fit.outputs.get(Kind::Xbd).unwrap()[row],
            oracle.result.fitted_values[i],
            seed,
            "independent fitted",
        );
        let xb = oracle.result.finite_fitted_values[i] + oracle.constant.unwrap_or(0.);
        check(
            PREDICTION,
            fit.outputs.get(Kind::Xb).unwrap()[row],
            xb,
            seed,
            "independent xb",
        );
        check(
            PREDICTION,
            fit.outputs.get(Kind::Absorbed).unwrap()[row],
            oracle.result.fitted_values[i] - xb,
            seed,
            "independent d",
        );
    }
    for (row, &reason) in oracle.drops.iter().enumerate() {
        assert_eq!(
            fit.outputs.get(Kind::DropReason).unwrap()[row],
            reason as f64
        );
        if reason != 0 {
            for column in &fit.outputs.columns {
                if column.kind != Kind::DropReason {
                    assert!(column.values[row].is_nan());
                }
            }
        }
    }
    if !fit.covariance.psd.adjusted {
        independent_covariance(fit, oracle, seed);
    }
}
fn independent_covariance(fit: &ols::OlsFit, oracle: &dense::Independent, seed: usize) {
    let result = &oracle.result;
    let kept = &result.kept_regressors;
    let finite = kept.len();
    let reported = oracle.constant.is_some();
    let coordinates = kept
        .iter()
        .map(|&i| oracle.slots[i])
        .chain(reported.then_some(fit.coefficients.len() - 1))
        .collect::<Vec<_>>();
    assert_eq!(
        coordinates, fit.coordinates,
        "seed {seed}, independent coordinates"
    );
    let covariance = match fit.vce {
        vce::Kind::Iid => &result.iid_vce,
        vce::Kind::Robust if reported => result
            .normalized_reported_intercept_hc1_vce
            .as_ref()
            .unwrap(),
        vce::Kind::Robust => &result.hc1_vce,
        vce::Kind::Cluster if reported => result
            .normalized_reported_intercept_clustered_vce
            .as_ref()
            .unwrap(),
        vce::Kind::Cluster => result.clustered_vce.as_ref().unwrap(),
    };
    let denominator = if fit.vce == vce::Kind::Cluster {
        fit.dof.correction
    } else {
        fit.dof.structural
    };
    let scale = result.residual_degrees_of_freedom as f64 / denominator as f64;
    for (i, &left) in coordinates.iter().enumerate() {
        for (j, &right) in coordinates.iter().enumerate() {
            let expected = if fit.vce == vce::Kind::Iid && (i == finite || j == finite) {
                let sigma = result.weighted_rss / denominator as f64;
                if i == finite && j == finite {
                    sigma
                        * (1. / oracle.weight_sum
                            + (0..finite)
                                .map(|a| {
                                    (0..finite)
                                        .map(|b| {
                                            oracle.means[kept[a]]
                                                * result.bread[(a, b)]
                                                * oracle.means[kept[b]]
                                        })
                                        .sum::<f64>()
                                })
                                .sum::<f64>())
                } else {
                    let row = i.min(j);
                    -sigma
                        * (0..finite)
                            .map(|a| result.bread[(row, a)] * oracle.means[kept[a]])
                            .sum::<f64>()
                }
            } else {
                covariance[(i, j)] * scale
            };
            check(
                INFERENCE,
                fit.covariance.values[(left, right)],
                expected,
                seed,
                "independent V",
            );
        }
    }
}
fn campaign(start: usize, count: usize) {
    let mut covariance_conflicts = 0;
    let mut reference_rejections = 0;
    let mut scale_rejections = 0;
    let mut roots = 0;
    for seed in start..start + count {
        let case = case(seed);
        let expected = match previous(&case) {
            Ok(fit) => Some(fit),
            Err(e) => {
                match e.message.as_str() {
                    "categorical absorbed-DoF slope row is inconsistent" => {
                        assert_eq!(e.code, old::ErrorCode::InvalidArgument);
                        assert_eq!(case.dof, 3);
                        assert!(case
                            .request
                            .absorbed_terms
                            .iter()
                            .any(|e| !e.slope_sources.is_empty()));
                        reference_rejections += 1;
                    }
                    "prepared OLS finite-score regressor scale is not finite and positive" => {
                        assert_eq!(e.code, old::ErrorCode::InvalidData);
                        assert!(case
                            .request
                            .absorbed_terms
                            .iter()
                            .any(|e| !e.slope_sources.is_empty()));
                        scale_rejections += 1;
                    }
                    _ => panic!("seed {seed}, legacy: {e}"),
                }
                None
            }
        };
        let oracle = dense::independent(&case);
        let (model, loaded) = current(&case);
        let request = Request {
            vce: Some(if case.vce >= 2 {
                vce::Kind::Cluster
            } else if case.vce == 0 && case.loaded.probability_weight.is_none() {
                vce::Kind::Iid
            } else {
                vce::Kind::Robust
            }),
            dof: [
                DofPolicy::None,
                DofPolicy::FirstPair,
                DofPolicy::Pairwise,
                DofPolicy::Continuous,
            ][case.dof],
            outputs: outputs::Request {
                residuals: true,
                xb: true,
                xbd: true,
                absorbed: true,
                drop_reason: true,
                saved: model
                    .absorbed_terms
                    .iter()
                    .enumerate()
                    .flat_map(|(effect, term)| {
                        (0..usize::from(term.intercept) + term.slope_sources.len())
                            .map(move |channel| (effect, channel))
                    })
                    .collect(),
            },
        };
        let options = Options {
            solver: Solver::Auto,
            threads: 1,
            tolerance: 1e-12,
            ..Options::for_estimator(Estimator::Ols)
        };
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            let options = Options { solver, ..options };
            let result =
                ols::fit_requested(&model, &loaded, &options, &request, Resources::default());
            if solver == Solver::Cmg
                && model
                    .absorbed_terms
                    .iter()
                    .any(|e| !e.slope_sources.is_empty())
            {
                assert_eq!(
                    result.unwrap_err().code,
                    fereg_engine::ErrorCode::Unsupported
                );
                continue;
            }
            let fit = result.unwrap_or_else(|e| panic!("seed {seed}, {solver:?}: {e}"));
            let stripes = case
                .request
                .model
                .terms
                .iter()
                .map(|t| t.canonical_token.clone())
                .chain(oracle.constant.map(|_| "_cons".into()))
                .collect::<Vec<_>>();
            assert_eq!(fit.stripes, stripes);
            roots += 1;
            compare_independent(&fit, &oracle, seed);
            saved_effects(&fit, &case, seed);
            if let Some(expected) = &expected {
                compare(&fit, expected, seed);
                if expected.psd.is_some_and(|r| r.slope_block_restored) {
                    covariance_conflicts += 1;
                } else {
                    compare_inference(&fit, expected, seed);
                }
            }
            if seed % 31 == 0 {
                let other = ols::fit_requested(
                    &model,
                    &loaded,
                    &Options {
                        threads: 8,
                        ..options
                    },
                    &request,
                    Resources::default(),
                )
                .unwrap();
                science_identity(&fit, &other);
                assert_eq!(fit.outputs.columns.len(), other.outputs.columns.len());
                for (a, b) in fit.outputs.columns.iter().zip(&other.outputs.columns) {
                    assert_eq!(a.kind, b.kind);
                    assert_eq!(bits(&a.values), bits(&b.values));
                }
                let off = ols::fit_requested(
                    &model,
                    &loaded,
                    &options,
                    &Request {
                        outputs: outputs::Request::default(),
                        vce: request.vce,
                        dof: request.dof,
                    },
                    Resources::default(),
                )
                .unwrap();
                assert!(off.outputs.columns.is_empty());
                science_identity(&fit, &off);
            }
        }
    }
    eprintln!("{count} designs: {roots} supported roots, {covariance_conflicts} pending covariance comparisons, {reference_rejections} frozen slope-report and {scale_rejections} frozen finite-score scale rejections");
}
fn science_identity(left: &ols::OlsFit, right: &ols::OlsFit) {
    assert_eq!(left.stripes, right.stripes);
    assert_eq!(left.coefficients, right.coefficients);
    assert_eq!(left.statuses, right.statuses);
    assert_eq!(left.coordinates, right.coordinates);
    assert_eq!(left.covariance.values, right.covariance.values);
    assert_eq!(
        bits(&left.covariance.standard_errors),
        bits(&right.covariance.standard_errors)
    );
    assert_eq!(bits(&left.t_statistics), bits(&right.t_statistics));
    assert_eq!(left.covariance.correction, right.covariance.correction);
    assert_eq!(left.covariance.psd, right.covariance.psd);
    assert_eq!(
        left.covariance.cluster_counts,
        right.covariance.cluster_counts
    );
    assert_eq!(
        left.covariance.subset_counts,
        right.covariance.subset_counts
    );
    assert_eq!(left.diagnostics, right.diagnostics);
    assert_eq!(left.dof, right.dof);
    assert_eq!(left.absorbed_dof, right.absorbed_dof);
    assert_eq!(left.wald, right.wald);
    assert_eq!(left.statistics, right.statistics);
    assert_eq!(left.sample, right.sample);
    assert_eq!(left.weights, right.weights);
}
fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}
fn saved_effects(fit: &ols::OlsFit, case: &Case, seed: usize) {
    if case.request.absorbed_terms.is_empty() {
        return;
    }
    for &row in &fit.sample.retained_rows {
        let mut total = 0.;
        for (effect, term) in case.request.absorbed_terms.iter().enumerate() {
            if term.intercept {
                total += fit.outputs.get(Kind::Saved { effect, channel: 0 }).unwrap()[row];
            }
            for index in 0..term.slope_sources.len() {
                total += fit
                    .outputs
                    .get(Kind::Saved {
                        effect,
                        channel: usize::from(term.intercept) + index,
                    })
                    .unwrap()[row];
            }
        }
        check(
            PREDICTION,
            total,
            fit.outputs.get(Kind::Absorbed).unwrap()[row],
            seed,
            "saved FE sum",
        );
    }
}
#[test]
fn reported_constant_multiway_contract_difference_is_reproduced() {
    let seed = 12;
    let case = case(seed);
    let expected = previous(&case).unwrap();
    let report = expected.psd.unwrap();
    assert!(report.slope_block_restored);
    assert!(!report.strict_final_full_psd);
    assert!(report.minimum_final_full_eigenvalue < -2e-6);
    let (model, loaded) = current(&case);
    let fit = ols::fit_requested(
        &model,
        &loaded,
        &Options {
            threads: 1,
            tolerance: 1e-12,
            ..Options::for_estimator(Estimator::Ols)
        },
        &Request {
            vce: Some(vce::Kind::Cluster),
            dof: DofPolicy::None,
            ..Request::default()
        },
        Resources::default(),
    )
    .unwrap();
    let oracle = dense::independent(&case);
    let raw = oracle
        .result
        .normalized_reported_intercept_clustered_vce
        .as_ref()
        .unwrap();
    let scale = oracle.result.residual_degrees_of_freedom as f64 / fit.dof.correction as f64;
    for (&actual, &raw) in expected.raw_covariance.iter().zip(raw.as_slice()) {
        check(
            INFERENCE,
            actual,
            raw * scale,
            seed,
            "independent pre-repair covariance",
        );
    }
    let (full_repair, certificate) = vce::psd::apply_psd_policy(
        &expected.raw_covariance,
        expected.coordinates.len(),
        2,
        5e-10,
    )
    .unwrap();
    assert!(certificate.passed);
    assert!(fit.covariance.psd.passed);
    assert!(fit.covariance.psd.final_minimum_eigenvalue > -1e-15);
    let p = expected.coordinates.len();
    let mut difference = 0_f64;
    for (i, &left) in expected.coordinates.iter().enumerate() {
        for (j, &right) in expected.coordinates.iter().enumerate() {
            let actual = fit.covariance.values[(left, right)];
            check(
                INFERENCE,
                actual,
                full_repair[i * p + j],
                seed,
                "full repair",
            );
            difference = difference.max((actual - expected.covariance[i * p + j]).abs());
        }
    }
    assert!(difference > 1e-6);
    eprintln!("seed {seed}: max covariance disagreement={difference:.17e}");
    eprintln!(
        "seed {seed}: legacy raw min={:.17e}, final min={:.17e}",
        report.minimum_raw_full_eigenvalue, report.minimum_final_full_eigenvalue
    );
}
#[test]
fn complete_factor_ts_calls_match_oracles_and_available_legacy_results() {
    campaign(0, 320);
}
#[test]
#[ignore = "long complete-estimator legacy campaign"]
fn long_complete_factor_ts_calls_match_oracles_and_available_legacy_results() {
    campaign(320, 2000);
}
#[test]
fn complete_calls_apply_stata_factor_domains_after_availability() {
    let case = case(41);
    let (mut model, mut loaded) = current(&case);
    let options = Options {
        threads: 1,
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ols)
    };
    let baseline = ols::fit(&model, &loaded, &options).unwrap();
    let source = model
        .model
        .sources
        .iter()
        .position(|s| s.canonical_name == "f")
        .unwrap();
    let fereg_engine::data::LoadedSource::Numeric(factor) = &mut loaded.sources[source] else {
        unreachable!()
    };
    for row in [0, 1, 2, 3, 4, 5, 6, 7, 8, 10] {
        factor.values[row] = -0.5;
    }
    let fit = ols::fit(&model, &loaded, &options).unwrap();
    assert_eq!(fit.sample, baseline.sample);
    assert_eq!(fit.statuses, baseline.statuses);
    assert_eq!(fit.coefficients, baseline.coefficients);
    assert_eq!(fit.covariance.values, baseline.covariance.values);
    let row = fit.sample.retained_rows[0];
    let fereg_engine::data::LoadedSource::Numeric(factor) = &mut loaded.sources[source] else {
        unreachable!()
    };
    factor.values[row] = -0.5;
    for omitted in [false, true] {
        for term in &mut model.model.terms {
            if term
                .components
                .iter()
                .any(|c| c.source.get() as usize - 1 == source)
            {
                term.whole_column_omitted |= omitted;
            }
        }
        assert_eq!(
            ols::fit(&model, &loaded, &options)
                .unwrap_err()
                .return_code(),
            452
        );
    }
    let (model, mut loaded) = current(&fixture::case(17));
    let last = loaded.candidate.len() - 1;
    let baseline = ols::fit(&model, &loaded, &options).unwrap();
    assert_eq!(baseline.sample.drop_reasons[last].code(), 8);
    let source = model
        .model
        .sources
        .iter()
        .position(|s| s.canonical_name == "f")
        .unwrap();
    let fereg_engine::data::LoadedSource::Numeric(factor) = &mut loaded.sources[source] else {
        unreachable!()
    };
    factor.values[last] = -0.5;
    assert_eq!(
        ols::fit(&model, &loaded, &options)
            .unwrap_err()
            .return_code(),
        452
    );
}
#[test]
fn frozen_continuous_slope_report_rejection_has_an_independent_regression() {
    let seed = 294;
    let case = case(seed);
    let error = previous(&case).err().unwrap();
    assert_eq!(error.code, old::ErrorCode::InvalidArgument);
    assert_eq!(
        error.message,
        "categorical absorbed-DoF slope row is inconsistent"
    );
    let oracle = dense::independent(&case);
    let (model, loaded) = current(&case);
    for solver in [Solver::Auto, Solver::Lsmr] {
        let fit = ols::fit_requested(
            &model,
            &loaded,
            &Options {
                threads: 1,
                solver,
                tolerance: 1e-12,
                ..Options::for_estimator(Estimator::Ols)
            },
            &Request {
                vce: Some(vce::Kind::Robust),
                dof: DofPolicy::Continuous,
                outputs: outputs::Request {
                    residuals: true,
                    xb: true,
                    xbd: true,
                    absorbed: true,
                    drop_reason: true,
                    ..outputs::Request::default()
                },
            },
            Resources::default(),
        )
        .unwrap();
        compare_independent(&fit, &oracle, seed);
    }
}
#[test]
fn frozen_retained_singleton_factor_rejection_has_an_independent_regression() {
    let seed = 66;
    let case = case(seed);
    let error = previous(&case).err().unwrap();
    assert_eq!(error.code, old::ErrorCode::InvalidData);
    assert_eq!(
        error.message,
        "prepared OLS finite-score regressor scale is not finite and positive"
    );
    let oracle = dense::independent(&case);
    let (model, loaded) = current(&case);
    for solver in [Solver::Auto, Solver::Lsmr] {
        let fit = ols::fit_requested(
            &model,
            &loaded,
            &Options {
                threads: 1,
                solver,
                tolerance: 1e-12,
                ..Options::for_estimator(Estimator::Ols)
            },
            &Request {
                vce: Some(vce::Kind::Iid),
                dof: DofPolicy::None,
                outputs: outputs::Request {
                    residuals: true,
                    xb: true,
                    xbd: true,
                    absorbed: true,
                    drop_reason: true,
                    ..outputs::Request::default()
                },
            },
            Resources::default(),
        )
        .unwrap();
        compare_independent(&fit, &oracle, seed);
    }
}
