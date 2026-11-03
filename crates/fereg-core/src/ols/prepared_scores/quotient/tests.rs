use super::*;
use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, ComponentKind, LevelState, MetadataFlag,
    RawTermKind, SourceEntry, SourceId, TermId, TermOrder,
};
use crate::ols::{
    fit_prepared_model_clustered_with_loaded_moments,
    fit_prepared_model_robust_with_loaded_moments, prepare_model, LoadedNumericColumn,
    OlsClusterTermV1, OlsInputBinding, OlsInputRole, PreparedOlsClusteredConfig, PreparedOlsData,
    PreparedOlsExecutionMode, PreparedOlsRobustConfig,
};

struct Cube {
    prepared: PreparedOlsData,
    mean: f64,
    robust_variance: f64,
}

fn cube(weighted: bool) -> Cube {
    cube_variant(weighted, 1, false)
}

fn cube_variant(weighted: bool, repeats: usize, omitted_slots: bool) -> Cube {
    let (mut request, mut loaded) = crate::ols::prepared_clusters::tests::fixture();
    let z = SourceId::new(4).unwrap();
    let cluster = SourceId::new(5).unwrap();
    let mut term = request.ols.model.terms[0].clone();
    term.id = TermId::new(2).unwrap();
    term.order = TermOrder::new(2).unwrap();
    term.coefficient_slot = CoefficientSlot::new(2).unwrap();
    term.canonical_token = "c.z".to_owned();
    term.components[0].source = z;
    if omitted_slots {
        let mut base = request.ols.model.terms[0].clone();
        base.id = TermId::new(2).unwrap();
        base.order = TermOrder::new(2).unwrap();
        base.coefficient_slot = CoefficientSlot::new(2).unwrap();
        base.whole_column_omitted = true;
        base.raw_kind = RawTermKind::Factor;
        base.canonical_token = "1.a".to_owned();
        let component = &mut base.components[0];
        component.source = SourceId::new(2).unwrap();
        component.kind = ComponentKind::Categorical;
        component.level_state = LevelState::Present;
        component.level_value = 1.;
        component.base = MetadataFlag::True;
        component.omitted = MetadataFlag::True;
        component.raw_operator = "1".to_owned();
        request.ols.model.terms.push(base);
        let mut absorbed = request.ols.model.terms[0].clone();
        absorbed.id = TermId::new(3).unwrap();
        absorbed.order = TermOrder::new(3).unwrap();
        absorbed.coefficient_slot = CoefficientSlot::new(3).unwrap();
        absorbed.canonical_token = "c.a".to_owned();
        absorbed.components[0].source = SourceId::new(2).unwrap();
        request.ols.model.terms.push(absorbed);
        term.id = TermId::new(4).unwrap();
        term.order = TermOrder::new(4).unwrap();
        term.coefficient_slot = CoefficientSlot::new(4).unwrap();
    }
    request.ols.model.terms.push(term);
    for (id, name) in [(z, "z"), (cluster, "cluster")] {
        request.ols.model.sources.push(SourceEntry {
            id,
            canonical_name: name.to_owned(),
        });
        request.ols.bindings.push(OlsInputBinding {
            stata_var_position: request.ols.bindings.len() as u32 + 1,
            role: OlsInputRole::NumericSource(id),
        });
    }
    request.ols.include_intercept = true;
    request.ols.absorbed_terms = (2..=3)
        .map(|id| AbsorbedTerm {
            id: AbsorbTermId::new(id - 1).unwrap(),
            intercept: true,
            categorical_sources: vec![SourceId::new(id).unwrap()],
            slope_sources: Vec::new(),
            label: format!("g{id}"),
            saved_name: String::new(),
        })
        .collect();
    request.clusters = vec![OlsClusterTermV1 {
        label: "sign cube".to_owned(),
        sources: vec![cluster],
    }];
    let mut columns = [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    let mut outcome = Vec::new();
    let mut weights = Vec::new();
    for g in 1..=10 {
        for h in 1..=10 {
            for sign in 0..8 {
                let u = if sign & 1 == 0 { -1. } else { 1. };
                let v = if sign & 2 == 0 { -1. } else { 1. };
                let w = if sign & 4 == 0 { -1. } else { 1. };
                columns[0].push(1e8 * g as f64 + u);
                columns[1].push(g as f64);
                columns[2].push(h as f64);
                columns[3].push(v);
                columns[4].push(sign as f64);
                outcome.push(5. * g as f64 + 7. * h as f64 + 2. * u - 3. * v + w);
                weights.push(if weighted {
                    (1 + (g + 2 * h) % 7) as f64
                } else {
                    1.
                });
            }
        }
    }
    for values in &mut columns {
        *values = values.repeat(repeats);
    }
    outcome = outcome.repeat(repeats);
    weights = weights.repeat(repeats);
    let observations = outcome.len();
    let mass: f64 = weights.iter().sum();
    let mean = columns[0]
        .iter()
        .zip(&weights)
        .map(|(x, w)| x * w)
        .sum::<f64>()
        / mass;
    let robust_variance = (observations as f64 / (observations - 21) as f64)
        * weights.iter().map(|w| w * w).sum::<f64>()
        / mass.powi(2);
    let column = |values: Vec<f64>| LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    };
    loaded.candidate = vec![true; observations].into();
    loaded.outcome = column(outcome);
    loaded.numeric_sources = columns
        .into_iter()
        .enumerate()
        .map(|(index, values)| (SourceId::new(index as u32 + 1).unwrap(), column(values)))
        .collect();
    if weighted {
        request.ols.bindings.push(OlsInputBinding {
            stata_var_position: request.ols.bindings.len() as u32 + 1,
            role: OlsInputRole::AnalyticWeight,
        });
        loaded.analytic_weight = Some(column(weights));
    }
    Cube {
        prepared: prepare_model(&request.ols, &request.clusters, &loaded, false).unwrap(),
        mean,
        robust_variance,
    }
}

fn check_covariance(values: &[f64], variance: f64, mean: f64) {
    let expected = [
        variance,
        0.,
        -mean * variance,
        0.,
        variance,
        0.,
        -mean * variance,
        0.,
        (1. + mean * mean) * variance,
    ];
    assert_eq!(values.len(), expected.len());
    // The reported constant contains -mean*beta_x. Verify every entry in
    // bounded parameter units using D V D, D=diag(1,1,1/max(1,|mean|)).
    // This keeps slope checks strict while treating a zero slope/constant
    // cross term according to the actual units of its two coordinates.
    let scales = [1., 1., 1. / mean.abs().max(1.)];
    for (index, (&actual, expected)) in values.iter().zip(expected).enumerate() {
        let actual_scaled = actual * scales[index / 3] * scales[index % 3];
        let expected_scaled = expected * scales[index / 3] * scales[index % 3];
        assert!(
            (actual_scaled - expected_scaled).abs() <= 1e-10,
            "index={index}, mean={mean:.17e}, actual={actual:.17e}, expected={expected:.17e}, normalized error={:.17e}",
            actual_scaled - expected_scaled,
        );
    }
}

#[test]
fn quotient_robust_covariance_matches_the_weighted_sign_cube() {
    for weighted in [false, true] {
        let data = cube(weighted);
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let mut config = PreparedOlsRobustConfig::default();
            config.fit.execution.mode = mode;
            let result =
                fit_prepared_model_robust_with_loaded_moments(&data.prepared, config, None)
                    .unwrap_or_else(|error| panic!("weighted={weighted}, mode={mode:?}: {error}"));
            assert!(result.fit().uses_quotient_inference());
            assert!(result.certificate().passed);
            assert_eq!(result.fit().finite_rank(), 2);
            check_covariance(
                result.covariance().values(),
                data.robust_variance,
                data.mean,
            );
            assert_relative(
                result.statistics().f_statistic.unwrap(),
                13. / (2. * data.robust_variance),
                1e-10,
            );
        }
    }
}

#[test]
fn quotient_cluster_covariance_matches_independent_sign_groups() {
    // Each sign triple is one cluster spanning all 100 FE cells. Its three
    // quotient score sums are equal-magnitude orthogonal sign patterns.
    // Thus uncorrected covariance is I/8, with no numerical projection oracle.
    for weighted in [false, true] {
        let data = cube(weighted);
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let mut config = PreparedOlsClusteredConfig::default();
            config.fit.execution.mode = mode;
            let result =
                fit_prepared_model_clustered_with_loaded_moments(&data.prepared, config, None)
                    .unwrap_or_else(|error| panic!("weighted={weighted}, mode={mode:?}: {error}"));
            assert!(result.fit().uses_quotient_inference());
            assert!(result.certificate().passed);
            assert_eq!(result.fit().finite_rank(), 2);
            let variance = (799. / 779.) * (8. / 7.) / 8.;
            check_covariance(result.covariance().values(), variance, data.mean);
            assert_relative(
                result.statistics().f_statistic.unwrap(),
                13. / (2. * variance),
                1e-10,
            );
        }
    }
}

#[test]
fn quotient_map_admits_its_additional_square_and_retained_fe_product() {
    let data = cube(true);
    for retain in [false, true] {
        let mut config = crate::ols::PreparedOlsFitConfig::default();
        config.absorb.solver = crate::fe::FeSolverKind::Map;
        config.absorb.retain_coefficients = retain;
        config.allow_final_fe_recovery = retain;
        let mut fit = crate::ols::fit_prepared_model_coefficients(&data.prepared, config).unwrap();
        let square_bytes = (fit.bread().dimension().pow(2) * 8) as u128;
        let retained_bytes = fit
            .recovered_fe_coefficients()
            .map_or(0, |batch| batch.retained_bytes());
        assert_eq!(retained_bytes > 0, retain);
        let required = square_bytes + retained_bytes;
        let execution = 137;
        let before = fit.solver_workspace_peak();
        let injected = build_bread(&mut fit, Some(2), None, &mut FailMapReservation).unwrap_err();
        assert_eq!(injected.code, crate::error::ErrorCode::Allocation);
        assert_eq!(fit.solver_workspace_peak(), before);
        let mut reservation = SystemPreparedInferenceReservation;
        let refused = build_bread(
            &mut fit,
            Some(2),
            Some(crate::memory::FeSolverMemoryBudget {
                execution_bytes: execution,
                usable_bytes: execution + required - 1,
            }),
            &mut reservation,
        )
        .unwrap_err();
        assert_eq!(refused.code, crate::error::ErrorCode::Allocation);
        let admitted = build_bread(
            &mut fit,
            Some(2),
            Some(crate::memory::FeSolverMemoryBudget {
                execution_bytes: execution,
                usable_bytes: execution + required,
            }),
            &mut reservation,
        )
        .unwrap()
        .unwrap();
        assert_eq!(admitted[2], 0.);
        assert_eq!(admitted[5], 0.);
        assert_eq!(
            admitted[8],
            1. / fit.bread().weight_scale().reference_weight_sum
        );
    }
}

fn assert_relative(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.),
        "actual={actual:.17e}, expected={expected:.17e}"
    );
}

struct FailMapReservation;
impl PreparedInferenceReservation for FailMapReservation {
    fn reserve<T>(
        &mut self,
        _target: &mut Vec<T>,
        _elements: usize,
        label: &'static str,
    ) -> Result<()> {
        assert_eq!(label, "unable to reserve quotient inference map");
        Err(allocation("injected quotient map reservation failure"))
    }
}

#[test]
fn quotient_covariance_preserves_base_and_absorbed_slots() {
    let data = cube_variant(true, 1, true);
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let mut config = PreparedOlsRobustConfig::default();
        config.fit.execution.mode = mode;
        let result =
            fit_prepared_model_robust_with_loaded_moments(&data.prepared, config, None).unwrap();
        use crate::ols::PreparedOlsFitStatus::*;
        assert_eq!(
            result
                .fit()
                .coefficients()
                .iter()
                .map(|c| c.status)
                .collect::<Vec<_>>(),
            [
                Kept,
                SyntaxOmitted,
                CollinearWithFixedEffects,
                Kept,
                NormalizedAbsorbedIntercept
            ]
        );
        assert_eq!(
            result
                .covariance_coordinates()
                .iter()
                .map(|c| c.coefficient_slot)
                .collect::<Vec<_>>(),
            [
                CoefficientSlot::new(1).unwrap(),
                CoefficientSlot::new(4).unwrap(),
                CoefficientSlot::new(5).unwrap()
            ]
        );
        assert!(result.certificate().passed);
        check_covariance(
            result.covariance().values(),
            data.robust_variance,
            data.mean,
        );
    }
}

#[test]
fn quotient_inference_repeats_and_agrees_across_worker_counts() {
    // 25,600 rows cross the 16,384-row robust reduction boundary. Replication
    // scales N and HC1; the eight cluster score patterns remain unchanged.
    let data = cube_variant(true, 32, false);
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let mut prior_robust = None;
        let mut prior_clustered = None;
        for workers in [1, 4] {
            let mut robust_config = PreparedOlsRobustConfig::default();
            robust_config.fit.execution.mode = mode;
            robust_config.fit.execution.threads =
                crate::ThreadConfig::from_requested(workers, true).unwrap();
            let run_robust = || {
                fit_prepared_model_robust_with_loaded_moments(&data.prepared, robust_config, None)
                    .unwrap()
            };
            let robust = run_robust();
            let repeated = run_robust();
            assert_eq!(robust.covariance(), repeated.covariance());
            assert_eq!(robust.fit().coefficients(), repeated.fit().coefficients());
            assert_eq!(robust.fit().residuals(), repeated.fit().residuals());
            assert_eq!(robust.certificate(), repeated.certificate());
            assert!(robust.certificate().passed);
            check_covariance(
                robust.covariance().values(),
                data.robust_variance,
                data.mean,
            );
            let identities = (
                robust.fit().kept_generated_columns().to_vec(),
                robust.fit().dropped_generated_columns().to_vec(),
            );
            if let Some(prior) = prior_robust {
                assert_eq!(identities, prior);
            }
            prior_robust = Some(identities);

            let cluster_config = PreparedOlsClusteredConfig {
                fit: robust_config.fit,
                ..PreparedOlsClusteredConfig::default()
            };
            let run_cluster = || {
                fit_prepared_model_clustered_with_loaded_moments(
                    &data.prepared,
                    cluster_config,
                    None,
                )
                .unwrap()
            };
            let clustered = run_cluster();
            let repeated = run_cluster();
            assert_eq!(clustered.covariance(), repeated.covariance());
            assert_eq!(
                clustered.fit().coefficients(),
                repeated.fit().coefficients()
            );
            assert_eq!(clustered.fit().residuals(), repeated.fit().residuals());
            assert_eq!(clustered.certificate(), repeated.certificate());
            assert!(clustered.certificate().passed);
            let variance = (25_599. / 25_579.) * (8. / 7.) / 8.;
            check_covariance(clustered.covariance().values(), variance, data.mean);
            let identities = (
                clustered.fit().kept_generated_columns().to_vec(),
                clustered.fit().dropped_generated_columns().to_vec(),
            );
            if let Some(prior) = prior_clustered {
                assert_eq!(identities, prior);
            }
            prior_clustered = Some(identities);
        }
    }
}
