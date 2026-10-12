use super::super::prepared_fit::fit_prepared_ols_coefficients_view_map_for_inference_with_cache_limit;
use super::*;
use crate::data::WeightKind;
use crate::error::ErrorCode;
use crate::ols::prepare_model;

struct CountingReservation {
    calls: usize,
    fail_at: Option<usize>,
    undersize: bool,
}

impl CountingReservation {
    const fn succeeds() -> Self {
        Self {
            calls: 0,
            fail_at: None,
            undersize: false,
        }
    }

    const fn fails_at(call: usize) -> Self {
        Self {
            calls: 0,
            fail_at: Some(call),
            undersize: false,
        }
    }

    const fn undersized() -> Self {
        Self {
            calls: 0,
            fail_at: None,
            undersize: true,
        }
    }
}

impl PreparedInferenceReservation for CountingReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        _label: &'static str,
    ) -> Result<()> {
        let call = self.calls;
        self.calls += 1;
        if self.fail_at == Some(call) {
            return Err(allocation(
                "injected prepared clustered inference reservation failure",
            ));
        }
        let requested = if self.undersize && elements > 0 {
            elements - 1
        } else {
            elements
        };
        target
            .try_reserve_exact(requested)
            .map_err(|_| allocation("test prepared clustered reservation failed"))
    }
}

#[test]
fn outcome_overlap_preserves_clustered_statistics_and_covariance() {
    let (mut request, mut loaded) = super::super::prepared_clusters::tests::fixture();
    request.ols.include_intercept = true;
    request.ols.absorbed_terms = vec![crate::metadata::AbsorbedTerm {
        id: crate::metadata::AbsorbTermId::new(1).unwrap(),
        intercept: true,
        categorical_sources: vec![request.ols.model.sources[1].id],
        slope_sources: Vec::new(),
        label: "a".to_owned(),
        saved_name: String::new(),
    }];
    loaded.candidate = loaded
        .candidate
        .iter()
        .copied()
        .collect::<Vec<_>>()
        .repeat(2049)
        .into();
    loaded.outcome.values = loaded.outcome.values.repeat(2049);
    loaded.outcome.missing = loaded.outcome.missing.repeat(2049);
    for (_, column) in &mut loaded.numeric_sources {
        column.values = column.values.repeat(2049);
        column.missing = column.missing.repeat(2049);
    }
    let n = loaded.candidate.len();
    for kind in [
        None,
        Some(WeightKind::Analytic),
        Some(WeightKind::Probability),
        Some(WeightKind::Frequency),
    ] {
        let (mut request, mut loaded) = (request.clone(), loaded.clone());
        if let Some(kind) = kind {
            let column = super::super::LoadedNumericColumn {
                values: (0..n)
                    .map(|r| {
                        if kind == WeightKind::Frequency {
                            (1 + r % 7) as f64
                        } else {
                            0.125 + (r % 17) as f64 / 11.0
                        }
                    })
                    .collect(),
                missing: vec![false; n],
            };
            let role = match kind {
                WeightKind::Analytic => {
                    loaded.analytic_weight = Some(column);
                    super::super::OlsInputRole::AnalyticWeight
                }
                WeightKind::Probability => {
                    loaded.probability_weight = Some(column);
                    super::super::OlsInputRole::ProbabilityWeight
                }
                WeightKind::Frequency => {
                    loaded.frequency_weight = Some(column);
                    super::super::OlsInputRole::FrequencyWeight
                }
                _ => unreachable!(),
            };
            request.ols.bindings.push(super::super::OlsInputBinding {
                stata_var_position: request.ols.bindings.len() as u32 + 1,
                role,
            });
        }
        let prepared = prepare_model(&request.ols, &request.clusters, &loaded, false).unwrap();
        let mut config = PreparedOlsClusteredConfig::default();
        config.fit.execution.threads = crate::ThreadConfig::from_requested(16, true).unwrap();
        let actual =
            fit_prepared_model_clustered_with_loaded_moments(&prepared, config, None).unwrap();
        let (fit, scratch) =
            super::super::prepared_fit::fit_prepared_ols_coefficients_view_map_for_inference(
                prepared.ols(),
                config.fit,
            )
            .unwrap();
        let expected = ThreadTeam::new(config.fit.execution.threads)
            .install(|| {
                compose_clustered_view(
                    &prepared,
                    fit,
                    config,
                    &mut SystemPreparedInferenceReservation,
                    ClusteredPerturbations::default(),
                    scratch,
                )
            })
            .unwrap()
            .unwrap();
        assert_eq!(actual.statistics, expected.statistics);
        assert_eq!(actual.covariance, expected.covariance);
        assert_eq!(actual.certificate, expected.certificate);
        assert_eq!(actual.dof, expected.dof);
        assert_eq!(actual.fit.coefficients(), expected.fit.coefficients());
    }
}

#[test]
fn strided_score_storage_matches_packed_clustered_inference() {
    let (mut request, loaded) = super::super::prepared_clusters::tests::fixture();
    request.ols.include_intercept = true;
    let prepared = prepare_model(&request.ols, &request.clusters, &loaded, false)
        .expect("prepared clustered fixture");
    let config = PreparedOlsClusteredConfig::default();
    let packed = fit_prepared_model_clustered_with_loaded_moments(&prepared, config, None)
        .expect("packed clustered inference");
    let fit = fit_prepared_ols_coefficients_view_map_for_inference_with_cache_limit(
        prepared.ols(),
        config.fit,
        0,
    )
    .expect("strided clustered coefficient fit");
    let mut reservation = CountingReservation::succeeds();
    let strided = compose(
        &prepared,
        fit,
        config,
        &mut reservation,
        ClusteredPerturbations::default(),
    )
    .expect("strided clustered inference");
    assert!(strided.certificate().passed);
    assert_eq!(packed.fit().coefficients(), strided.fit().coefficients());
    for (actual, expected) in strided
        .covariance()
        .values()
        .iter()
        .zip(packed.covariance().values())
    {
        let scale = actual.abs().max(expected.abs()).max(1.0);
        assert!((actual - expected).abs() <= 2e-11 * scale);
    }
}

#[test]
fn low_memory_small_cluster_table_matches_legacy_group_influences() {
    let (mut request, mut loaded) = super::super::prepared_clusters::tests::fixture();
    request.ols.include_intercept = true;
    let original = loaded.clone();
    loaded.candidate = original
        .candidate
        .iter()
        .copied()
        .collect::<Vec<_>>()
        .repeat(512)
        .into();
    loaded.outcome.values = original.outcome.values.repeat(512);
    loaded.outcome.missing = original.outcome.missing.repeat(512);
    for ((_, target), (_, source)) in loaded
        .numeric_sources
        .iter_mut()
        .zip(&original.numeric_sources)
    {
        target.values = source.values.repeat(512);
        target.missing = source.missing.repeat(512);
    }
    let prepared = prepare_model(&request.ols, &request.clusters, &loaded, false)
        .expect("large clustered fixture");
    let mut config = PreparedOlsClusteredConfig::default();
    config.fit.execution.mode = super::super::PreparedOlsExecutionMode::Low;
    let low = fit_prepared_model_clustered_with_loaded_moments(&prepared, config, None)
        .expect("bounded score table");
    assert!(low.certificate().passed);
    // Every subset also runs the independent pre-extraction scratch reducer
    // inside the provider and requires bit-identical meats and group counts.
    let mut count = CountingReservation::succeeds();
    fit_with_reservation(&prepared, config, &mut count).expect("count new-route reservations");
    for fail_at in 0..count.calls {
        let mut failing = CountingReservation::fails_at(fail_at);
        assert!(fit_with_reservation(&prepared, config, &mut failing).is_err());
    }
}

#[test]
fn low_memory_batched_cluster_tables_match_legacy_and_fail_closed() {
    let (mut request, mut loaded) = super::super::prepared_clusters::tests::fixture();
    request.ols.include_intercept = true;
    let sources = request.clusters[0].sources.clone();
    request.clusters = sources
        .into_iter()
        .enumerate()
        .map(|(index, source)| super::super::OlsClusterTermV1 {
            label: format!("cluster_{index}"),
            sources: vec![source],
        })
        .collect();
    let original = loaded.clone();
    loaded.candidate = original
        .candidate
        .iter()
        .copied()
        .collect::<Vec<_>>()
        .repeat(512)
        .into();
    loaded.outcome.values = original.outcome.values.repeat(512);
    loaded.outcome.missing = original.outcome.missing.repeat(512);
    for ((_, target), (_, source)) in loaded
        .numeric_sources
        .iter_mut()
        .zip(&original.numeric_sources)
    {
        target.values = source.values.repeat(512);
        target.missing = source.missing.repeat(512);
    }
    let prepared = prepare_model(&request.ols, &request.clusters, &loaded, false)
        .expect("large clustered fixture");
    let mut config = PreparedOlsClusteredConfig::default();
    config.fit.execution.mode = super::super::PreparedOlsExecutionMode::Low;
    let low = fit_prepared_model_clustered_with_loaded_moments(&prepared, config, None)
        .expect("bounded score table");
    assert!(low.certificate().passed);
    // Every subset also runs the independent pre-extraction scratch reducer
    // inside the provider and requires bit-identical meats and group counts.
    let mut count = CountingReservation::succeeds();
    fit_with_reservation(&prepared, config, &mut count).expect("count new-route reservations");
    for fail_at in 0..count.calls {
        let mut failing = CountingReservation::fails_at(fail_at);
        assert!(fit_with_reservation(&prepared, config, &mut failing).is_err());
    }
}

#[test]
fn every_clustered_inference_reservation_failure_and_undersize_fails_closed() {
    let (request, loaded) = super::super::prepared_clusters::tests::fixture();
    let prepared = prepare_model(&request.ols, &request.clusters, &loaded, false)
        .expect("prepared clustered fixture");
    let mut count = CountingReservation::succeeds();
    fit_with_reservation(&prepared, PreparedOlsClusteredConfig::default(), &mut count)
        .expect("count clustered inference reservations");
    assert!(count.calls >= 20);
    for fail_at in 0..count.calls {
        let mut reservation = CountingReservation::fails_at(fail_at);
        let error = fit_with_reservation(
            &prepared,
            PreparedOlsClusteredConfig::default(),
            &mut reservation,
        )
        .expect_err("injected clustered inference allocation must fail");
        assert_eq!(error.code, ErrorCode::Allocation, "fail_at={fail_at}");
    }
    let error = fit_with_reservation(
        &prepared,
        PreparedOlsClusteredConfig::default(),
        &mut CountingReservation::undersized(),
    )
    .expect_err("undersized clustered inference allocation must fail");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn every_clustered_scientific_mutation_fails_closed() {
    let (mut request, loaded) = super::super::prepared_clusters::tests::fixture();
    request.ols.include_intercept = true;
    let prepared = prepare_model(&request.ols, &request.clusters, &loaded, false)
        .expect("prepared clustered fixture");
    for perturbation in [
        ClusteredPerturbations {
            score_design: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            residual_dof: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            subset_sign: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            subset_groups: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            uncorrected_covariance: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            corrected_covariance: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            correction: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            final_covariance: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            covariance_symmetry: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            standard_error: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            psd_state: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            rss: true,
            ..ClusteredPerturbations::default()
        },
        ClusteredPerturbations {
            f_statistic: true,
            ..ClusteredPerturbations::default()
        },
    ] {
        let mut reservation = SystemPreparedInferenceReservation;
        let error = compose_for_test(&prepared, &mut reservation, perturbation)
            .expect_err("injected clustered scientific corruption must fail");
        assert_eq!(error.code, ErrorCode::InvalidData, "{perturbation:?}");
    }
}

fn compose_for_test<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsData,
    reservation: &mut R,
    perturbations: ClusteredPerturbations,
) -> Result<CertifiedPreparedOlsClustered> {
    let config = PreparedOlsClusteredConfig::default();
    let fit = fit_prepared_ols_coefficients_view_map_for_inference_with_cache_limit(
        prepared.ols(),
        config.fit,
        0,
    )?;
    compose(prepared, fit, config, reservation, perturbations)
}

#[test]
fn singular_wald_slope_block_is_explicitly_undefined() {
    let (request, loaded) = super::super::prepared_clusters::tests::fixture();
    let prepared = prepare_model(&request.ols, &request.clusters, &loaded, false)
        .expect("prepared clustered fixture");
    let result = fit_prepared_model_clustered_with_loaded_moments(
        &prepared,
        PreparedOlsClusteredConfig::default(),
        None,
    )
    .expect("clustered fixture fit");
    let dimension = result.covariance.dimension();
    let zero = PreparedOlsClusteredCovariance {
        correction: result.covariance.correction,
        raw_values: vec![0.0; dimension * dimension],
        values: vec![0.0; dimension * dimension],
        standard_errors: vec![0.0; dimension],
        psd: PreparedOlsClusteredPsdReport {
            state: PreparedOlsClusteredPsdState::OneWayStrict,
            full_adjusted: false,
            slope_block_restored: false,
            slope_block_adjusted: false,
            full_clipped_eigenvalues: 0,
            slope_clipped_eigenvalues: 0,
            minimum_raw_full_eigenvalue: 0.0,
            minimum_pre_restore_full_eigenvalue: 0.0,
            minimum_raw_slope_eigenvalue: None,
            minimum_restored_slope_eigenvalue: None,
            minimum_final_full_eigenvalue: 0.0,
            pre_restore_full_rank: 0,
            restored_slope_rank: None,
            strict_final_full_psd: true,
            transform_reconstruction_relative_error: 0.0,
            eigensystem_relative_error: 0.0,
            orthogonality_relative_error: 0.0,
        },
    };
    let mut reservation = SystemPreparedInferenceReservation;
    let wald = build_wald(
        prepared.ols(),
        result.fit(),
        &zero,
        result.dof().finite_model_dof,
        PreparedOlsClusteredConfig::default().certificate_tolerance,
        &mut reservation,
    )
    .expect("singular Wald policy");
    assert_eq!(wald.statistic, None);
    assert_eq!(wald.solve_relative_infinity_norm, 0.0);
}
