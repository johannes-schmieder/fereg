use super::*;
use crate::data::{DropReason, WeightKind};
use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder,
    ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent,
    TermId, TermOrder, TimeSeriesState,
};
use crate::ols::{OlsInputBinding, OlsInputRole};
use crate::ppml::prepared_irls::{
    reset_canonical_frame_refresh_count, take_canonical_frame_refresh_count,
    take_cmg_early_memory_peaks, take_fused_strict_witness_count,
};
use crate::ppml::{PpmlFallbackKind, PPML_STEADY_STATE_AUDIT_ITERATIONS};
use crate::ppml_separation::{MethodStatus, SeparationDropReason, SeparationMethods};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

static FIT_PHASE_BEGINS: AtomicUsize = AtomicUsize::new(0);

fn input_factor_hint(request: &OlsRequestV1, loaded: &LoadedOlsInput) -> PreparedOlsLoadHint {
    let ids = PreparedOlsLoadHint::source_ids(request).unwrap();
    let rows = loaded.candidate.len();
    PreparedOlsLoadHint::prepare(
        ids.iter().map(|id| {
            let (_, column) = loaded
                .numeric_sources
                .iter()
                .find(|(source, _)| source == id)
                .unwrap();
            (*id, column.values.as_slice(), column.missing.as_slice())
        }),
        rows,
        PreparedOlsLoadHint::workspace_bytes(rows, ids.len()).unwrap(),
    )
    .unwrap()
}

#[test]
fn load_hint_preserves_first_plan_certificates_and_domain_precedence() {
    let exact_inference = |mut report: crate::ppml::PpmlInferenceResult| {
        // Omitted inference coordinates intentionally contain NaNs. Compare
        // every numeric array by IEEE bits, then every remaining report field
        // by structural equality; nothing is removed from the witness.
        let bits = [
            std::mem::take(&mut report.vce.covariance),
            std::mem::take(&mut report.vce.standard_errors),
            std::mem::take(&mut report.vce.z_statistics),
        ]
        .map(|values| values.into_iter().map(f64::to_bits).collect::<Vec<_>>());
        (report, bits)
    };
    static DECISIONS: std::sync::Mutex<Vec<super::super::PpmlMemoryPreflightObservation>> =
        std::sync::Mutex::new(Vec::new());
    fn record(event: super::super::PpmlMemoryPreflightEvent) -> Result<()> {
        if let super::super::PpmlMemoryPreflightEvent::Decision(observation) = event {
            DECISIONS.lock().unwrap().push(observation);
        }
        Ok(())
    }
    for workers in [1, 4, 16] {
        for case in 0..7 {
            let (mut request, mut loaded) = fixture(2, false, true, true, true, false);
            let hint = input_factor_hint(&request.ols, &loaded.ols);
            request.linear.execution.threads =
                crate::ThreadConfig::from_requested(workers, false).unwrap();
            request.memory_preflight_probe = Some(record);
            if case == 6 {
                request.memory.available_bytes = Some(1);
            }
            match case {
                1 => loaded.ols.numeric_sources[0].1.missing[0] = true,
                2 => loaded.adjustment.as_mut().unwrap().missing[0] = true,
                3 => loaded.ols.probability_weight.as_mut().unwrap().values[0] = 0.,
                4 | 5 => {
                    loaded.ols.outcome.values[0] = -1.;
                    loaded.ols.numeric_sources[0].1.missing[0] = case == 4;
                }
                _ => (),
            }
            DECISIONS.lock().unwrap().clear();
            let expected = fit_prepared_ppml(request.clone(), loaded.clone());
            let expected_plans = std::mem::take(&mut *DECISIONS.lock().unwrap());
            // Both sides receive equal owned capacities: the fit's raw-owner
            // memory envelope deliberately includes spare Vec/String capacity.
            let actual = fit_prepared_ppml_interruptible(
                request.clone(),
                (loaded.clone(), Some(hint)),
                &mut |_| Ok(()),
            );
            assert_eq!(
                *DECISIONS.lock().unwrap(),
                expected_plans,
                "workers={workers}, case={case}"
            );
            match (expected, actual) {
                (Err(expected), Err(actual)) => assert_eq!(actual, expected),
                (Ok(expected), Ok(actual)) => {
                    assert_eq!(actual.coefficients(), expected.coefficients());
                    assert_eq!(
                        exact_inference(actual.inference().clone()),
                        exact_inference(expected.inference().clone()),
                    );
                    assert_eq!(actual.sample(), expected.sample());
                    assert_eq!(actual.output(), expected.output());
                    assert_eq!(actual.convergence(), expected.convergence());
                    let mut actual_separation = actual.separation().clone();
                    let mut expected_separation = expected.separation().clone();
                    // Observed duration is not a scientific replay field.
                    // Keep every separation decision and certificate exact.
                    for report in [&mut actual_separation, &mut expected_separation] {
                        for method in &mut report.methods {
                            method.elapsed_seconds = 0.0;
                        }
                    }
                    assert_eq!(actual_separation, expected_separation);
                    assert_eq!(actual.certificate(), expected.certificate());
                    assert_eq!(actual.memory().plan, expected.memory().plan);
                }
                (expected, actual) => panic!("mismatched hint result: {expected:?} / {actual:?}"),
            }
        }
    }
}

#[test]
fn load_hint_requires_full_rows_and_bounds_coexisting_descriptors() {
    let (request, loaded) = fixture(2, false, false, false, false, false);
    let make = || input_factor_hint(&request.ols, &loaded.ols);
    let n = loaded.ols.candidate.len();
    let classified = PpmlNumericClassification {
        n_original: n,
        n_candidate: n,
        n_active: n,
    };
    let hint = make();
    assert!(hint.adoption_workspace_bytes() > hint.owned_bytes());
    let mut metadata = super::super::PpmlMetadataMemory {
        generation_bytes_per_row: hint.adoption_workspace_bytes().div_ceil(n as u128),
        ..Default::default()
    };
    assert!(admit_load_hint(Some(hint), classified, metadata).is_some());
    metadata.generation_bytes_per_row -= 1;
    assert!(admit_load_hint(Some(make()), classified, metadata).is_none());
    metadata.generation_bytes_per_row += 1;
    assert!(admit_load_hint(
        Some(make()),
        PpmlNumericClassification {
            n_active: n - 1,
            ..classified
        },
        metadata
    )
    .is_none());
    metadata.generation_bytes_per_row = u128::MAX;
    assert!(admit_load_hint(Some(make()), classified, metadata).is_none());
}

#[test]
fn generation_parent_bound_covers_factors_clusters_slopes_and_rebases() {
    use crate::ppml_separation::PreparedSeparationRebuilder;
    let mut fixtures = Vec::new();
    for effects in [0, 1, 2, 4] {
        for clustered in [false, true] {
            for probability in [false, true] {
                fixtures.push(fixture(effects, false, clustered, probability, true, false));
            }
        }
    }
    fixtures.push(factor_rebase_fixture());
    let (mut slope, data) = fixture(2, false, true, true, true, false);
    slope.ols.absorbed_terms[0]
        .slope_sources
        .push(SourceId::new(1).unwrap());
    fixtures.push((slope, data));
    let (mut interaction, data) = fixture(2, false, true, false, false, false);
    interaction.ols.absorbed_terms[0]
        .categorical_sources
        .push(SourceId::new(3).unwrap());
    interaction.clusters[0]
        .sources
        .push(SourceId::new(2).unwrap());
    fixtures.push((interaction, data));
    for (request, loaded) in fixtures {
        let input = borrowed_numeric_input(&request, &loaded).unwrap();
        let view = input.as_input();
        let classified = classify_ppml_numeric_data(&view).unwrap();
        let classification = classified.classification();
        let dimensions = conservative_preflight_dimensions_from_classification(
            &request,
            classification,
            input.numeric_sources.len(),
            input.offset.is_some() || input.exposure.is_some(),
            loaded.ols.probability_weight.is_some(),
            0,
        )
        .unwrap();
        let plan = plan_ppml_memory(dimensions, request.memory).unwrap();
        let bound = plan.selected_inventory().prepared_generation_bytes;
        let numeric = select_ppml_numeric_data(classified).unwrap();
        drop(input);
        let mut owner = PreparedPpmlModelOwner::from_selection(
            request.ols,
            request.clusters,
            loaded.ols,
            numeric,
            loaded.adjustment,
        )
        .unwrap()
        .with_generation_memory_bound(bound);
        let rows = owner.candidate_original_rows();
        let old = owner.rebuild_generation(&rows, 0).unwrap();
        let new = owner
            .rebuild_generation(&rows[..rows.len() - 2], 1)
            .unwrap();
        assert!(old.owned_buffer_bytes() <= bound);
        assert!(new.owned_buffer_bytes() <= bound);
        // Old/new coexist through commit; neither can use the other's bound.
        assert!(old.owned_buffer_bytes() + new.owned_buffer_bytes() <= 2 * bound);
    }
}

#[test]
fn metadata_admission_counts_nested_recipes_labels_and_partial_saves() {
    use crate::ols::PreparedOlsCategoricalComponent;
    use std::mem::size_of;
    let (mut request, _) = fixture(2, true, true, false, false, false);
    let original =
        super::super::PpmlMetadataMemory::from_request(&request.ols, &request.clusters).unwrap();
    // A label is copied by both prepared and final cluster metadata.
    request.clusters[0].label.push_str(&"long".repeat(1_000));
    let labels =
        super::super::PpmlMetadataMemory::from_request(&request.ols, &request.clusters).unwrap();
    assert_eq!(
        labels.prepared_nested_bytes - original.prepared_nested_bytes,
        4_000
    );
    assert_eq!(labels.result_bytes - original.result_bytes, 4_000);
    // Request metadata owns nested terms; executable interaction recipes
    // allocate their own typed components in both candidate and rebase.
    let term = &mut request.ols.model.terms[0];
    term.raw_kind = RawTermKind::Interaction;
    term.components.push(term.components[0].clone());
    let recipes =
        super::super::PpmlMetadataMemory::from_request(&request.ols, &request.clusters).unwrap();
    let added = (2 * size_of::<PreparedOlsCategoricalComponent>()) as u128;
    assert_eq!(
        recipes.prepared_nested_bytes - labels.prepared_nested_bytes,
        added
    );
    // Finite recipes are consumed in place; adding an interaction does
    // not recreate a transient model or clone source metadata.
    assert_eq!(
        recipes.preparation_transient_bytes,
        labels.preparation_transient_bytes
    );
    request.ols.absorbed_terms[1].saved_name = "second".into();
    assert_eq!(
        super::super::PpmlMetadataMemory::from_request(&request.ols, &request.clusters).unwrap(),
        recipes,
        "partial saving still reserves all declared channel descriptors and identities"
    );
    request.ols.model.terms[0].components[0]
        .raw_operator
        .push_str(&"x".repeat(4096));
    let operators =
        super::super::PpmlMetadataMemory::from_request(&request.ols, &request.clusters).unwrap();
    assert_eq!(
        operators.preparation_transient_bytes - recipes.preparation_transient_bytes,
        0,
        "borrowed availability metadata does not copy operator payloads"
    );
    assert_eq!(
        operators.prepared_nested_bytes,
        recipes.prepared_nested_bytes
    );
}

#[test]
fn positive_outcomes_bound_report_banks_across_routes_weights_and_singletons() {
    for route in [MemoryMode::Fast, MemoryMode::Low] {
        for weight in [
            None,
            Some(WeightKind::Frequency),
            Some(WeightKind::Probability),
        ] {
            for singletons in [false, true] {
                let (mut request, mut loaded) = fixture(
                    2,
                    false,
                    false,
                    weight == Some(WeightKind::Probability),
                    false,
                    false,
                );
                request.memory.requested = route;
                request.memory.poolsize = 1;
                request.separation = SeparationConfig::default();
                if weight == Some(WeightKind::Frequency) {
                    request.ols.bindings.push(OlsInputBinding {
                        stata_var_position: 6,
                        role: OlsInputRole::FrequencyWeight,
                    });
                    loaded.ols.frequency_weight = Some(column(&[
                        2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 1.0, 1.0,
                    ]));
                }
                if singletons {
                    loaded.ols.numeric_sources[1].1.values[11] = 99.0;
                }
                let result = fit_prepared_ppml(request, loaded).expect("positive PPML");
                assert_eq!(
                    result
                        .memory()
                        .plan
                        .preflight_dimensions
                        .separation_generation_capacity,
                    2
                );
                assert_eq!(result.separation().methods.capacity(), 8);
                assert_eq!(result.separation().generation_hashes.capacity(), 2);
                assert_eq!(result.separation().generations, usize::from(singletons));
                assert_eq!(
                    result.sample().original_rows().len(),
                    if singletons { 10 } else { 12 }
                );
                assert!(result.certificate().passed);
            }
        }
    }
}

#[test]
fn excluded_zero_keeps_conservative_admission_but_compact_report_bank() {
    let (mut request, mut loaded) = fixture(2, false, false, false, false, false);
    request.separation = SeparationConfig::default();
    loaded.ols.candidate.explicit_mut().unwrap()[11] = false;
    loaded.ols.outcome.values[11] = 0.0;
    let result = fit_prepared_ppml(request, loaded).expect("excluded zero");
    assert_eq!(
        result
            .memory()
            .plan
            .preflight_dimensions
            .separation_generation_capacity,
        11
    );
    assert_eq!(result.separation().methods.capacity(), 8);
    assert_eq!(result.separation().generation_hashes.capacity(), 2);
    assert_eq!(result.sample().original_rows().len(), 10);
}

#[test]
fn positive_outcome_scan_remains_interruptible_before_admission() {
    let (request, loaded) = fixture(2, false, false, false, false, false);
    let mut checks = 0;
    let error = fit_prepared_ppml_interruptible(request, loaded, &mut |point| {
        if point == PpmlInterruptPoint::Preparation {
            checks += 1;
            if checks == 2 {
                return Err(FeregError::new(
                    ErrorCode::Interrupted,
                    "preflight interrupt",
                ));
            }
        }
        Ok(())
    })
    .expect_err("scan interrupt");
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert_eq!(checks, 2);
}

#[test]
fn publication_capacity_failure_is_allocation_r909() {
    let error =
        fallible_filled::<f64>(usize::MAX, 0.0, "unable to reserve PPML publication output")
            .expect_err("impossible publication capacity must fail before allocation");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(error.return_code(), 909);

    assert_eq!(
        fallible_copy(&[2_usize, 5, 9], "copy").expect("small fallible copy"),
        [2, 5, 9]
    );
    assert_eq!(
        fallible_filled(3, None::<usize>, "fill").expect("small fallible fill"),
        [None, None, None]
    );
    let string_error =
        fallible_string_with_capacity(usize::MAX, "unable to reserve PPML publication label")
            .expect_err("impossible publication string capacity must fail before allocation");
    assert_eq!(string_error.code, ErrorCode::Allocation);
    assert_eq!(string_error.return_code(), 909);
}

fn count_fit_phase_begins(_phase: PpmlPhase, event: super::super::PpmlPhaseAllocatorEvent) -> u64 {
    if matches!(event, super::super::PpmlPhaseAllocatorEvent::Begin) {
        FIT_PHASE_BEGINS.fetch_add(1, Ordering::SeqCst);
    }
    0
}

#[test]
fn unsupported_public_solver_topology_fails_before_fit_lifetime_allocation() {
    let (mut request, loaded) = fixture(1, true, false, false, false, false);
    request.solver = PpmlSolverFamily::Lsmr;
    request.phase_allocation_probe = Some(count_fit_phase_begins);
    FIT_PHASE_BEGINS.store(0, Ordering::SeqCst);

    let error = fit_prepared_ppml(request, loaded).expect_err("one-FE PPML LSMR must fail closed");
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert_eq!(error.return_code(), 498);
    assert!(error.message.contains("at least two"));
    assert_eq!(
        FIT_PHASE_BEGINS.load(Ordering::SeqCst),
        0,
        "PPML solver rejection must precede preparation and every publication phase",
    );
}

#[test]
fn public_specialized_solvers_reject_memory_low() {
    for solver in [PpmlSolverFamily::Lsmr, PpmlSolverFamily::Cmg] {
        let (mut request, _loaded) = fixture(2, false, false, false, false, false);
        request.solver = solver;
        request.memory.requested = MemoryMode::Low;
        let error = validate_public_solver_request(&request)
            .expect_err("specialized PPML solvers must reject memory(low)");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert_eq!(error.return_code(), 498);
    }
}

#[test]
fn automatic_family_matches_explicit_solver_and_preserves_requested_identity() {
    for effects in 0..=3 {
        for zero_active in [false, true] {
            if zero_active && effects == 0 {
                continue;
            }
            let (mut request, mut loaded) = fixture(effects, true, false, true, true, false);
            if zero_active {
                make_finite_control_collinear_with_first_effect(&mut loaded);
            }
            request.solver = PpmlSolverFamily::Auto;
            let selected = PpmlSolverFamily::Map;
            let mut explicit = request.clone();
            explicit.solver = selected;
            let expected = fit_prepared_ppml(explicit, loaded.clone()).expect("explicit fit");
            let automatic = fit_prepared_ppml(request, loaded).expect("automatic fit");
            assert_eq!(automatic.solver().requested, PpmlSolverFamily::Auto);
            assert_eq!(automatic.solver().selected, selected);
            assert_eq!(automatic.solver().effective, selected);
            assert_eq!(
                automatic.solver().selected_engine,
                expected.solver().selected_engine
            );
            assert_eq!(automatic.solver().checkpoint, expected.solver().checkpoint);
            assert_eq!(automatic.solver().fallback_count, 0);
            assert_ppml_results_close(&automatic, &expected);
            assert_eq!(automatic.coefficients(), expected.coefficients());
            assert_eq!(automatic.sample(), expected.sample());
            assert_eq!(automatic.likelihood(), expected.likelihood());
            assert_eq!(automatic.inference().dof, expected.inference().dof);
            assert_eq!(automatic.inference().wald, expected.inference().wald);
            assert_eq!(
                automatic.inference().vce.covariance,
                expected.inference().vce.covariance
            );
            assert_eq!(
                automatic.inference().vce.certificate,
                expected.inference().vce.certificate
            );
            for (left, right) in automatic
                .inference()
                .vce
                .standard_errors
                .iter()
                .chain(&automatic.inference().vce.z_statistics)
                .zip(
                    expected
                        .inference()
                        .vce
                        .standard_errors
                        .iter()
                        .chain(&expected.inference().vce.z_statistics),
                )
            {
                // Omitted coordinates carry the same intentional NaN.
                assert_eq!(left.to_bits(), right.to_bits());
            }
            assert_eq!(automatic.output(), expected.output());
            assert_eq!(automatic.fixed_effects(), expected.fixed_effects());
            assert_eq!(automatic.clusters(), expected.clusters());
        }
    }
}

#[test]
fn automatic_low_memory_retains_certified_map() {
    let (mut request, loaded) = fixture(2, true, false, false, false, false);
    request.solver = PpmlSolverFamily::Auto;
    request.memory.requested = MemoryMode::Low;
    let automatic = fit_prepared_ppml(request, loaded).expect("automatic low-memory fit");
    assert_eq!(automatic.solver().requested, PpmlSolverFamily::Auto);
    assert_eq!(automatic.solver().selected, PpmlSolverFamily::Map);
    assert_eq!(automatic.solver().fallback_count, 0);
}

#[test]
fn automatic_specialized_fit_is_interruption_atomic() {
    for stop in [
        PpmlInterruptPoint::IrlsIteration,
        PpmlInterruptPoint::StrictFinalSolve,
        PpmlInterruptPoint::Inference,
        PpmlInterruptPoint::FeRecovery,
    ] {
        let (mut request, loaded) = fixture(2, true, false, false, false, false);
        request.solver = PpmlSolverFamily::Auto;
        let error = fit_prepared_ppml_interruptible(request, loaded, &mut |point| {
            if point == stop {
                Err(FeregError::new(
                    ErrorCode::Interrupted,
                    "automatic fit interrupted",
                ))
            } else {
                Ok(())
            }
        })
        .expect_err("interrupted auto must not return a result");
        assert_eq!(error.code, ErrorCode::Interrupted);
    }
}

#[test]
fn explicit_lsmr_matches_map_and_reports_its_concrete_engine() {
    for effects in [2, 3] {
        let (baseline_request, baseline_loaded) =
            fixture(effects, false, false, false, false, false);
        let baseline = fit_prepared_ppml(baseline_request, baseline_loaded)
            .expect("production MAP PPML oracle");
        let (mut request, loaded) = fixture(effects, false, false, false, false, false);
        request.solver = PpmlSolverFamily::Lsmr;
        request.memory.requested = MemoryMode::Fast;
        reset_canonical_frame_refresh_count();
        let candidate = fit_prepared_ppml(request, loaded).expect("public LSMR PPML");
        let canonical_refreshes = take_canonical_frame_refresh_count();
        assert_eq!(candidate.solver().requested, PpmlSolverFamily::Lsmr);
        assert_eq!(candidate.solver().selected, PpmlSolverFamily::Lsmr);
        assert_eq!(
            candidate.solver().selected_engine,
            super::super::PpmlSolverEngine::LsmrJoint
        );
        assert_specialized_route_skips_ordinary_canonical_refresh(
            &candidate,
            canonical_refreshes,
            super::super::PpmlSolverEngine::LsmrJoint,
        );
        assert_ppml_results_close(&candidate, &baseline);
    }
}

#[test]
fn zero_active_control_lsmr_uses_joint_state_but_reports_fused_identity() {
    for effects in [2, 3] {
        let (baseline_request, mut baseline_loaded) =
            fixture(effects, false, false, false, false, false);
        make_finite_control_collinear_with_first_effect(&mut baseline_loaded);
        let baseline = fit_prepared_ppml(baseline_request, baseline_loaded)
            .expect("rank-zero production MAP PPML oracle");

        let (mut request, mut loaded) = fixture(effects, false, false, false, false, false);
        make_finite_control_collinear_with_first_effect(&mut loaded);
        request.solver = PpmlSolverFamily::Lsmr;
        request.memory.requested = MemoryMode::Fast;
        reset_canonical_frame_refresh_count();
        let candidate = fit_prepared_ppml(request, loaded)
            .expect("rank-zero public LSMR PPML through reusable joint state");
        let canonical_refreshes = take_canonical_frame_refresh_count();

        assert_eq!(candidate.solver().requested, PpmlSolverFamily::Lsmr);
        assert_eq!(candidate.solver().selected, PpmlSolverFamily::Lsmr);
        assert_eq!(
            candidate.solver().selected_engine,
            super::super::PpmlSolverEngine::LsmrFused,
        );
        assert_specialized_route_skips_ordinary_canonical_refresh(
            &candidate,
            canonical_refreshes,
            super::super::PpmlSolverEngine::LsmrFused,
        );
        assert_eq!(candidate.solver().engines()[2].ordinary_frames, 0);
        assert_ppml_results_close(&candidate, &baseline);
    }
}

#[test]
fn three_effect_lsmr_rebuilds_joint_state_after_a_strict_basis_refresh() {
    let (mut request, loaded) = fixture(3, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Lsmr;
    request.memory.requested = MemoryMode::Fast;
    super::super::prepared_irls::inject_strict_basis_change(true, false);
    let candidate = fit_prepared_ppml(request, loaded)
        .expect("three-effect LSMR must survive a strict basis refresh");
    assert!(candidate.convergence().converged);
    assert!(candidate.convergence().strict_profile_rejections >= 1);
    assert!(candidate.solver().engines()[2].ordinary_frames > 0);
}

#[test]
fn zero_active_control_lsmr_rebuilds_joint_state_after_a_strict_basis_refresh() {
    let (mut request, mut loaded) = fixture(3, false, false, false, false, false);
    make_finite_control_collinear_with_first_effect(&mut loaded);
    request.solver = PpmlSolverFamily::Lsmr;
    request.memory.requested = MemoryMode::Fast;
    super::super::prepared_irls::inject_strict_basis_change(true, false);
    let candidate = fit_prepared_ppml(request, loaded)
        .expect("zero-active-control LSMR must survive a strict basis refresh");
    assert!(candidate.convergence().converged);
    assert!(candidate.convergence().strict_profile_rejections >= 1);
    assert_eq!(
        candidate.solver().selected_engine,
        super::super::PpmlSolverEngine::LsmrFused,
    );
    assert!(candidate.solver().engines()[1].ordinary_frames > 0);
    assert_eq!(candidate.solver().engines()[2].ordinary_frames, 0);
}

#[test]
fn public_cmg_rejects_four_fixed_effect_dimensions() {
    let (mut request, _loaded) = fixture(4, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Cmg;
    let error = validate_public_solver_request(&request)
        .expect_err("public CMG must be limited to two or three FE dimensions");
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert!(error.message.contains("exactly two or three"));
}

#[test]
fn public_cmg_matches_the_map_ppml_oracle() {
    for effects in [2, 3] {
        let (baseline_request, baseline_loaded) =
            fixture(effects, false, false, false, false, false);
        let baseline = fit_prepared_ppml(baseline_request, baseline_loaded)
            .expect("production MAP PPML oracle");

        let (mut request, loaded) = fixture(effects, false, false, false, false, false);
        request.solver = PpmlSolverFamily::Cmg;
        request.memory.requested = MemoryMode::Fast;
        reset_canonical_frame_refresh_count();
        let candidate = fit_prepared_ppml(request, loaded).expect("public prepared CMG PPML");
        let canonical_refreshes = take_canonical_frame_refresh_count();
        assert_eq!(candidate.solver().requested, PpmlSolverFamily::Cmg);
        assert_eq!(candidate.solver().effective, PpmlSolverFamily::Cmg);
        assert_eq!(
            candidate.memory().fallback_mask & (1_u32 << PpmlFallbackKind::Route as u8),
            0,
            "the focused {effects}-FE fixture must execute CMG without MAP fallback",
        );
        assert_specialized_route_skips_ordinary_canonical_refresh(
            &candidate,
            canonical_refreshes,
            super::super::PpmlSolverEngine::CmgPrepared,
        );
        assert_ppml_results_close(&candidate, &baseline);
    }
}

#[test]
fn public_cmg_retries_only_the_failed_projection_rhs() {
    let (baseline_request, baseline_loaded) = fixture(2, false, false, false, false, false);
    let baseline =
        fit_prepared_ppml(baseline_request, baseline_loaded).expect("production MAP PPML oracle");
    let (mut request, loaded) = fixture(2, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Cmg;
    request.memory.requested = MemoryMode::Fast;
    super::super::categorical_fwl_cmg::inject_projection_certificate_failure_once(0);
    let candidate = fit_prepared_ppml(request, loaded).expect("retried fused CMG PPML");
    assert_eq!(
        super::super::categorical_fwl_cmg::take_total_retried_right_hand_sides(),
        1,
    );
    assert_eq!(
        candidate.memory().fallback_mask & (1_u32 << PpmlFallbackKind::Route as u8),
        0,
    );
    assert_ppml_results_close(&candidate, &baseline);
}

#[test]
fn public_cmg_numerical_failure_falls_back_to_map_permanently() {
    let (baseline_request, baseline_loaded) = fixture(2, false, false, false, false, false);
    let baseline =
        fit_prepared_ppml(baseline_request, baseline_loaded).expect("production MAP PPML oracle");
    let (mut request, loaded) = fixture(2, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Cmg;
    request.memory.requested = MemoryMode::Fast;
    super::super::categorical_fwl_cmg::inject_numerical_failure_once();
    let candidate = fit_prepared_ppml(request, loaded).expect("MAP fallback PPML");
    assert_ne!(
        candidate.memory().fallback_mask & (1_u32 << PpmlFallbackKind::Route as u8),
        0,
    );
    assert!(matches!(
        candidate.memory().linear_route,
        Some(FeSolverKind::Map | FeSolverKind::Lsmr)
    ));
    assert_eq!(candidate.solver().requested, PpmlSolverFamily::Cmg);
    assert_eq!(candidate.solver().effective, PpmlSolverFamily::Map);
    assert_eq!(candidate.solver().fallback_count, 1);
    assert_eq!(
        candidate.solver().fallback_reason,
        super::super::PpmlSolverFallbackReason::CmgNumericalNonconvergence
    );
    let solver = candidate.solver();
    let cmg = &solver.engines()[3];
    assert_eq!(cmg.ordinary_frames, 1);
    assert_eq!(cmg.retries, 0);
    assert_eq!(cmg.fallback_count, 1);
    assert_ppml_results_close(&candidate, &baseline);
}

#[test]
fn public_cmg_non_numerical_failures_remain_fatal() {
    for code in [
        ErrorCode::Allocation,
        ErrorCode::Internal,
        ErrorCode::InvalidArgument,
        ErrorCode::InvalidData,
        ErrorCode::Interrupted,
    ] {
        let (mut request, loaded) = fixture(2, false, false, false, false, false);
        request.solver = PpmlSolverFamily::Cmg;
        request.memory.requested = MemoryMode::Fast;
        super::super::categorical_fwl_cmg::inject_fatal_failure_once(code);
        let error = fit_prepared_ppml(request, loaded)
            .expect_err("non-numerical prepared CMG failure must not fall back to MAP");
        assert_eq!(error.code, code);
        assert!(error.message.contains("fatal failure"));
    }
}

#[test]
fn public_lsmr_checkpoint_workspace_is_required_before_canonical_work() {
    let (mut request, loaded) = fixture(3, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Lsmr;
    request.memory.requested = MemoryMode::Fast;
    let admitted = fit_prepared_ppml(request, loaded).expect("unbounded LSMR fixture");
    let plan = admitted.memory().plan;
    let usable = plan.selected_inventory().peak_bytes + plan.solver_workspace_bytes - 1;
    let headroom = 512_u128 * 1024 * 1024;
    assert!((usable + headroom) / 5 <= headroom);
    let (mut request, loaded) = fixture(3, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Lsmr;
    request.memory.requested = MemoryMode::Fast;
    request.memory.available_bytes = Some(usable + headroom);
    reset_canonical_frame_refresh_count();
    let error = fit_prepared_ppml(request, loaded).expect_err("full solver peak must fit");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("public PPML FE solver workspace"));
    assert_eq!(take_canonical_frame_refresh_count(), 0);
}

#[test]
fn automatic_structural_map_uses_admitted_shared_operations() {
    let (mut request, loaded) = fixture(3, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Lsmr;
    request.memory.requested = MemoryMode::Fast;
    let explicit = fit_prepared_ppml(request, loaded).expect("unbounded LSMR");
    let plan = explicit.memory().plan;
    let usable = plan.selected_inventory().peak_bytes + plan.solver_workspace_bytes - 1;
    let headroom = 512_u128 * 1024 * 1024;
    assert!((usable + headroom) / 5 <= headroom);
    let (mut request, loaded) = fixture(3, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Auto;
    request.memory.requested = MemoryMode::Fast;
    request.memory.available_bytes = Some(usable + headroom);
    let automatic = fit_prepared_ppml(request, loaded).expect("budget-admitted MAP");
    assert_eq!(automatic.solver().requested, PpmlSolverFamily::Auto);
    assert_eq!(automatic.solver().selected, PpmlSolverFamily::Map);
    assert_eq!(automatic.solver().fallback_count, 0);
    assert!(automatic.memory().plan.solver_workspace_bytes > 0);
    assert!(
        automatic.memory().plan.selected_inventory().peak_bytes
            + automatic.memory().plan.solver_workspace_bytes
            <= usable
    );
    // Shared numerical directions/checkpoints report their actual work.
    // Only the unselected explicit CMG implementation must remain unused.
    let report = automatic.solver();
    let cmg = &report.engines()[3];
    assert_eq!(cmg.iterations, 0);
    assert_eq!(cmg.retries, 0);
    assert_eq!(cmg.setup_seconds, 0.0);
    assert_ppml_results_close(&automatic, &explicit);
}

#[test]
fn public_ppml_rejects_cmg_sampling_before_preparation_or_policy_replacement() {
    for linear in [false, true] {
        for (samples, seed) in [(1, 0), (0, 1), (1, 1_u64 << 53)] {
            let (mut request, loaded) = fixture(2, false, false, false, false, false);
            let absorb = if linear {
                &mut request.linear.absorb
            } else {
                &mut request.ppml.absorb
            };
            absorb.solver = FeSolverKind::CmgSchur;
            absorb.cmg_schur_samples = samples;
            absorb.cmg_schur_seed = seed;
            let mut probes = 0;
            let error = fit_prepared_ppml_interruptible(request, loaded, &mut |_| {
                probes += 1;
                Ok(())
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidArgument);
            assert!(error.message.contains("sampling"));
            assert_eq!(probes, 0);
        }
    }
}

#[test]
fn public_cmg_intermediate_budget_rejection_precedes_route_observation_preparation() {
    let (mut admitted_request, admitted_loaded) = fixture(2, false, false, false, false, false);
    admitted_request.solver = PpmlSolverFamily::Cmg;
    admitted_request.memory.requested = MemoryMode::Fast;
    super::super::categorical_fwl_cmg::take_route_observation_preparations();
    let admitted = fit_prepared_ppml(admitted_request, admitted_loaded)
        .expect("unbounded prepared CMG fixture");
    assert!(super::super::categorical_fwl_cmg::take_route_observation_preparations() > 0);
    let (topology_only, complete) = take_cmg_early_memory_peaks().expect("CMG early memory peaks");
    let base = admitted.memory().plan.selected_inventory().peak_bytes;
    let solver = admitted.memory().plan.solver_workspace_bytes;
    assert!(
        solver > complete,
        "small specialized fits also admit the independent checkpoint workspace"
    );
    assert!(complete > topology_only);
    let usable = base
        .checked_add(topology_only)
        .expect("intermediate CMG usable-memory boundary");
    let headroom = 512_u128 * 1024 * 1024;
    let available = usable
        .checked_add(headroom)
        .expect("small fixture available-memory boundary");
    assert!(available / 5 <= headroom);

    let (mut request, loaded) = fixture(2, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Cmg;
    request.memory.requested = MemoryMode::Fast;
    request.memory.available_bytes = Some(available);
    super::super::categorical_fwl_cmg::take_route_observation_preparations();
    let error = fit_prepared_ppml(request, loaded)
        .expect_err("a topology-only CMG budget must fail before route preparation");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("public PPML FE solver workspace"));
    assert_eq!(
        super::super::categorical_fwl_cmg::take_route_observation_preparations(),
        0,
        "route-owned observation-scale topology must not start before budget admission",
    );
    assert_eq!(
        take_cmg_early_memory_peaks(),
        Some((topology_only, complete)),
        "the rejected fit must apply the same complete early CMG bound",
    );
}

#[test]
fn public_cmg_fallback_retains_retry_timing_and_certificate_evidence() {
    let (baseline_request, baseline_loaded) = fixture(2, false, false, false, false, false);
    let baseline =
        fit_prepared_ppml(baseline_request, baseline_loaded).expect("production MAP PPML oracle");
    let (mut request, loaded) = fixture(2, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Cmg;
    request.memory.requested = MemoryMode::Fast;
    super::super::categorical_fwl_cmg::inject_projection_certificate_failure_once(0);
    super::super::categorical_fwl_cmg::inject_post_projection_numerical_failure_once();

    let candidate = fit_prepared_ppml(request, loaded).expect("reported MAP fallback PPML");
    assert_eq!(
        super::super::categorical_fwl_cmg::take_total_retried_right_hand_sides(),
        1,
    );
    let solver = candidate.solver();
    let cmg = &solver.engines()[3];
    assert_eq!(cmg.ordinary_frames, 1);
    assert_eq!(cmg.retries, 1);
    assert!(cmg.iterations > 0);
    assert!(cmg.setup_seconds + cmg.solve_seconds + cmg.certification_seconds > 0.0);
    assert!(cmg.absolute_infinity_norm_max.is_some());
    assert!(cmg.relative_infinity_norm_max.is_some());
    assert!(cmg.relative_two_norm_max.is_some());
    assert!(cmg.projection_absolute_infinity_norm_max.is_some());
    assert!(cmg.backward_error_max.is_some());
    assert_eq!(cmg.fallback_count, 1);
    assert_eq!(solver.fallback_count, 1);
    assert_eq!(solver.effective, PpmlSolverFamily::Map);
    assert_ppml_results_close(&candidate, &baseline);
}

fn assert_specialized_route_skips_ordinary_canonical_refresh(
    candidate: &CertifiedPpmlResult,
    canonical_refreshes: usize,
    expected_engine: super::super::PpmlSolverEngine,
) {
    let solver = candidate.solver();
    let map = &solver.engines()[0];
    let specialized = solver
        .engines()
        .iter()
        .find(|report| report.engine == expected_engine)
        .expect("canonical solver report contains the selected engine row");
    assert!(specialized.ordinary_frames > 0);
    assert_eq!(map.ordinary_frames, 0);
    // Rank-zero external joint witnesses report the fused engine, whose
    // row also includes canonical checkpoints. They do not refresh the
    // canonical session and must be counted separately from those frames.
    let fused_witnesses = take_fused_strict_witness_count();
    assert_eq!(
        canonical_refreshes.saturating_add(1) + fused_witnesses,
        map.checkpoint_frames + solver.engines()[1].checkpoint_frames,
        "canonical frames and fused witnesses are attributed to their computing engines",
    );
    assert_eq!(solver.checkpoint, PpmlSolverFamily::Lsmr);
}

fn assert_ppml_results_close(candidate: &CertifiedPpmlResult, baseline: &CertifiedPpmlResult) {
    for (left, right) in candidate
        .coefficients()
        .values()
        .iter()
        .zip(baseline.coefficients().values())
    {
        assert!((left - right).abs() <= 1e-8 * left.abs().max(right.abs()).max(1.0));
    }
    for (left, right) in candidate.output().eta().iter().zip(baseline.output().eta()) {
        assert!((left - right).abs() <= 1e-8 * left.abs().max(right.abs()).max(1.0));
    }
    assert!(candidate.output().certificate().passed);
}

#[test]
fn profiled_final_products_match_canonical_and_reject_stale_authority() {
    use super::super::prepared_irls::profile_final_test_mode;
    for effects in [1, 2, 3] {
        for (memory, pool) in [
            (MemoryMode::Fast, 10000),
            (MemoryMode::Low, 1),
            (MemoryMode::Low, 2),
        ] {
            for threads in [1, 4] {
                for weighted in [false, true] {
                    let run = |mode| {
                        profile_final_test_mode(mode);
                        let (mut request, loaded) =
                            fixture(effects, true, true, weighted, true, false);
                        request.memory.requested = memory;
                        request.memory.poolsize = pool;
                        request.linear.execution.poolsize = pool;
                        request.linear.execution.threads =
                            crate::options::ThreadConfig::from_requested(threads, true).unwrap();
                        let result = fit_prepared_ppml(request, loaded);
                        let accepted = profile_final_test_mode(0);
                        (result.expect("profiled final products"), accepted)
                    };
                    let (baseline, count) = run(1);
                    assert_eq!(count, 0);
                    for mode in [0, 2, 3] {
                        let (candidate, count) = run(mode);
                        assert_eq!(count, usize::from(mode == 0));
                        assert_ppml_results_close(&candidate, &baseline);
                        for (left, right) in candidate
                            .inference()
                            .vce
                            .covariance
                            .iter()
                            .zip(&baseline.inference().vce.covariance)
                        {
                            assert!(
                                (left - right).abs() <= 1e-8 * left.abs().max(right.abs()).max(1.0)
                            );
                        }
                        assert!(candidate.inference().vce.certificate.passed);
                        assert!(
                            candidate
                                .output()
                                .saved_effects()
                                .unwrap()
                                .full_decomposition_certified
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn explicit_public_map_matches_one_effect_auto_bitwise() {
    let (mut baseline_request, baseline_loaded) = fixture(1, false, false, false, false, false);
    baseline_request.solver = PpmlSolverFamily::Auto;
    let baseline =
        fit_prepared_ppml(baseline_request, baseline_loaded).expect("production MAP PPML oracle");
    let (mut request, loaded) = fixture(1, false, false, false, false, false);
    request.solver = PpmlSolverFamily::Map;
    request.memory.requested = MemoryMode::Fast;
    let candidate = fit_prepared_ppml(request, loaded).expect("explicit public MAP PPML");
    assert_eq!(candidate.solver().requested, PpmlSolverFamily::Map);
    assert_eq!(candidate.memory().linear_route, Some(FeSolverKind::Map));
    for (left, right) in candidate
        .coefficients()
        .values()
        .iter()
        .zip(baseline.coefficients().values())
    {
        assert_eq!(left.to_bits(), right.to_bits());
    }
    assert_eq!(candidate.output().eta(), baseline.output().eta());
}

#[test]
fn interrupt_callback_may_be_non_send_and_runs_only_on_the_caller_thread() {
    let (request, loaded) = fixture(0, false, false, false, false, false);
    let caller = std::thread::current().id();
    let polls = Rc::new(Cell::new(0_usize));
    let saw_irls = Rc::new(Cell::new(false));
    let callback_polls = Rc::clone(&polls);
    let callback_saw_irls = Rc::clone(&saw_irls);
    let error = fit_prepared_ppml_interruptible(request, loaded, &mut |point| {
        assert_eq!(std::thread::current().id(), caller);
        callback_polls.set(callback_polls.get().saturating_add(1));
        if point == PpmlInterruptPoint::IrlsIteration {
            callback_saw_irls.set(true);
            Err(FeregError::new(
                ErrorCode::Interrupted,
                "injected caller-thread PPML interruption",
            ))
        } else {
            Ok(())
        }
    })
    .expect_err("the first IRLS caller-thread checkpoint must interrupt");

    assert_eq!(error.code, ErrorCode::Interrupted);
    assert!(saw_irls.get());
    assert!(polls.get() > 1);
}

#[test]
fn no_fe_constant_is_identified_with_a_real_robust_covariance_coordinate() {
    let (request, loaded) = fixture(0, false, false, false, false, false);
    let result = fit_prepared_ppml(request, loaded).expect("no-FE composite PPML");
    assert_eq!(
        result.coefficients().statuses(),
        &[
            PpmlCoefficientStatus::Identified,
            PpmlCoefficientStatus::Identified,
        ]
    );
    assert_eq!(result.coefficients().kept_full_slots(), &[0, 1]);
    assert_eq!(result.inference().dof.sandwich_rank, 2);
    assert_eq!(result.inference().dof.finite_within_rank, 2);
    assert_eq!(result.inference().dof.absorbed_rank, 0);
    assert_eq!(
        result.convergence().inner_iterations,
        result.convergence().outer_iterations + result.convergence().strict_profile_rejections + 2
    );
    assert_eq!(result.inference().vce.covariance.len(), 4);
    assert!(result.inference().vce.standard_errors[1].is_finite());
    assert!(result.inference().vce.standard_errors[1] > 0.0);
    assert!(result
        .memory()
        .timings
        .get(PpmlPhase::Publication)
        .is_some());
    assert!(result.certificate().passed);
}

#[test]
fn constant_factor_model_wald_is_basis_and_outcome_scale_invariant() {
    for memory in [MemoryMode::Fast, MemoryMode::Low] {
        for workers in [1, 4] {
            for scale in [1.0, 2.0] {
                let (mut request, loaded) = constant_wald_fixture(false, scale);
                request.memory.requested = memory;
                request.linear.execution.threads =
                    crate::options::ThreadConfig::from_requested(workers, true).unwrap();
                let result = fit_prepared_ppml(request, loaded).expect("constant-factor model");
                assert_eq!(
                    result.coefficients().statuses(),
                    &[
                        PpmlCoefficientStatus::Identified,
                        PpmlCoefficientStatus::Identified,
                        PpmlCoefficientStatus::NumericallyRankDeficient,
                    ]
                );
                assert!((result.coefficients().values()[0] - 3.0_f64.ln()).abs() < 1e-9);
                assert!((result.inference().vce.covariance[0] - 5. / 27.).abs() < 1e-9);
                assert_eq!(result.inference().wald.degrees_of_freedom, 1);
                assert!(
                    (result.inference().wald.statistic.unwrap()
                        - (27. / 5.) * 3.0_f64.ln().powi(2))
                    .abs()
                        < 1e-8
                );
                let reference = result.inference().wald.reference.as_ref().unwrap();
                assert!(reference.span_error() <= reference.span_tolerance());
                assert_eq!(reference.slot(), 1);
                for (row, &mean) in result.output().mean().iter().enumerate() {
                    assert!((mean - scale * if row < 3 { 1. } else { 3. }).abs() < 1e-8);
                }
            }
        }
    }
}

// The cellwise residuals sum to zero and are orthogonal to z=(1,2,3).
// Thus mu=(1,1,1,3,3,3) is an independent Poisson optimum. Direct dummy
// sandwich algebra gives V[x,x]=13/45 with slope-only FE, and 5/27 without.
fn constant_wald_fixture(slopes: bool, scale: f64) -> (PreparedPpmlRequestV1, LoadedPpmlInput) {
    let (mut request, mut loaded) = fixture(usize::from(slopes), false, false, false, false, false);
    let f = SourceId::new(if slopes { 3 } else { 2 }).unwrap();
    request.ols.model.sources.push(source(f, "f"));
    request
        .ols
        .model
        .terms
        .push(categorical_term(2, f, 1.0, false));
    request.ols.bindings.push(OlsInputBinding {
        stata_var_position: if slopes { 5 } else { 4 },
        role: OlsInputRole::NumericSource(f),
    });
    loaded.ols.candidate = vec![true; 6].into();
    loaded.ols.outcome = column(if slopes {
        &[1.25, 0.5, 1.25, 3.5, 2.0, 3.5]
    } else {
        &[0.5, 1.0, 1.5, 1.0, 3.0, 5.0]
    });
    loaded
        .ols
        .outcome
        .values
        .iter_mut()
        .for_each(|y| *y *= scale);
    loaded.ols.numeric_sources[0].1 = column(&[0., 0., 0., 1., 1., 1.]);
    loaded.ols.numeric_sources.push((f, column(&[1.; 6])));
    if slopes {
        loaded.ols.numeric_sources[1].1 = column(&[1., 1., 1., 2., 2., 2.]);
        let z = SourceId::new(4).unwrap();
        request.ols.model.sources.push(source(z, "z"));
        request.ols.bindings.push(OlsInputBinding {
            stata_var_position: 6,
            role: OlsInputRole::NumericSource(z),
        });
        request.ols.absorbed_terms[0].intercept = false;
        request.ols.absorbed_terms[0].slope_sources.push(z);
        loaded
            .ols
            .numeric_sources
            .push((z, column(&[1., 2., 3., 1., 2., 3.])));
    }
    (request, loaded)
}

#[test]
fn constant_wald_with_partial_slope_absorption_matches_dummy_sandwich() {
    for memory in [MemoryMode::Fast, MemoryMode::Low] {
        for workers in [1, 4] {
            let (mut request, loaded) = constant_wald_fixture(true, 1.0);
            request.memory.requested = memory;
            request.linear.execution.threads =
                crate::options::ThreadConfig::from_requested(workers, true).unwrap();
            let result = fit_prepared_ppml(request, loaded).expect("partly absorbed constant");
            assert_eq!(result.inference().wald.degrees_of_freedom, 1);
            assert!((result.coefficients().values()[0] - 3.0_f64.ln()).abs() < 1e-8);
            assert!((result.inference().vce.covariance[0] - 13.0 / 45.0).abs() < 1e-8);
            assert!(
                (result.inference().wald.statistic.unwrap() - (45.0 / 13.0) * 3.0_f64.ln().powi(2))
                    .abs()
                    < 1e-7
            );
            assert!(result.inference().wald.reference.is_some());
            for (row, &mean) in result.output().mean().iter().enumerate() {
                assert!((mean - if row < 3 { 1.0 } else { 3.0 }).abs() < 1e-8);
            }
        }
    }
}

#[test]
fn constant_wald_proof_interruption_returns_no_composite_result() {
    let (request, loaded) = constant_wald_fixture(false, 1.0);
    let caller = std::thread::current().id();
    let mut inference_started = false;
    let error = fit_prepared_ppml_interruptible(request, loaded, &mut |point| {
        assert_eq!(std::thread::current().id(), caller);
        inference_started |= point == PpmlInterruptPoint::Inference;
        if point == PpmlInterruptPoint::InferenceSubset && !inference_started {
            return Err(FeregError::new(
                ErrorCode::Interrupted,
                "constant proof interrupted",
            ));
        }
        Ok(())
    })
    .expect_err("constant proof must interrupt before inference/publication");
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert!(!inference_started);
}

#[test]
fn whole_factor_level_separation_rebuilds_on_the_original_axis() {
    for removed in [1.0, 3.0] {
        let (mut request, mut loaded) = factor_rebase_fixture();
        loaded.ols.outcome.missing.fill(false);
        let codes = &loaded.ols.numeric_sources[0].1.values;
        for (row, &level) in codes.iter().enumerate() {
            if level == removed {
                loaded.ols.outcome.values[row] = 0.0;
            }
        }
        request.output.mean = true;
        request.output.requested_observation_columns = 1;
        request.separation = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: true,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        let result =
            fit_prepared_ppml(request, loaded).expect("whole-level separated factor model");
        assert_eq!(result.separation().generations, 1);
        assert_eq!(
            result.separation().separated_count,
            if removed == 1.0 { 2 } else { 5 }
        );
        assert_eq!(result.coefficients().values().len(), 5);
        assert_eq!(
            result.coefficients().statuses()[0],
            PpmlCoefficientStatus::BaseLevel
        );
        assert_eq!(
            result.coefficients().statuses()[3],
            PpmlCoefficientStatus::SyntaxOmitted
        );
        if removed == 3.0 {
            assert_eq!(
                result.coefficients().statuses()[2],
                PpmlCoefficientStatus::NumericallyRankDeficient
            );
            assert_eq!(result.coefficients().values()[2], 0.0);
            for j in 0..5 {
                assert_eq!(result.inference().vce.covariance[2 * 5 + j], 0.0);
                assert_eq!(result.inference().vce.covariance[j * 5 + 2], 0.0);
            }
        }
        let expected_rows = if removed == 1.0 {
            (2..12).collect::<Vec<_>>()
        } else {
            (0..7).collect::<Vec<_>>()
        };
        assert_eq!(result.sample().original_rows(), expected_rows);
        for (row, &mean) in result.output().mean().iter().enumerate() {
            let original = result.sample().original_rows()[row];
            let (slot, expected) = if original < 2 {
                (None, 1.5)
            } else if original < 7 {
                (Some(1), 1.8)
            } else {
                (Some(2), 4.4)
            };
            let values = result.coefficients().values();
            let eta = values[4] + slot.map_or(0.0, |slot| values[slot]);
            assert!((eta.exp() - expected).abs() < 1e-9);
            assert!((mean - expected).abs() < 1e-9);
        }
        assert!(result
            .separation()
            .methods
            .iter()
            .any(|report| report.method == SeparationMethod::Simplex
                && report.status == MethodStatus::Certified));
        assert!(result.certificate().passed);
    }
}

#[test]
fn vanished_base_ppml_preserves_original_cell_means_and_robust_covariance() {
    let (mut request, loaded) = factor_rebase_fixture();
    request.output.mean = true;
    request.output.requested_observation_columns = 1;
    let result = fit_prepared_ppml(request, loaded).expect("original factor graph PPML");
    let values = result.coefficients().values();
    let covariance = &result.inference().vce.covariance;
    let contrasts = [[0.0, 1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 0.0, 1.0]];
    let log_means = [(9.0_f64 / 5.0).ln(), (22.0_f64 / 5.0).ln()];
    let variances = [28.0 / 729.0, 13.0 / 1089.0];
    for a in 0..2 {
        let eta = (0..5).map(|i| contrasts[a][i] * values[i]).sum::<f64>();
        assert!((eta - log_means[a]).abs() < 1e-9);
        for b in 0..2 {
            let variance = (0..5)
                .flat_map(|i| (0..5).map(move |j| (i, j)))
                .map(|(i, j)| contrasts[a][i] * covariance[i * 5 + j] * contrasts[b][j])
                .sum::<f64>();
            let expected = if a == b { variances[a] } else { 0.0 };
            assert!((variance - expected).abs() < 1e-9);
        }
    }
    for (row, &mean) in result.output().mean().iter().enumerate() {
        let expected = if row < 5 { 9.0 / 5.0 } else { 22.0 / 5.0 };
        assert!((mean - expected).abs() < 1e-9);
    }
    assert_eq!(result.inference().wald.degrees_of_freedom, 1);
    let independent_wald = (log_means[1] - log_means[0]).powi(2) / (variances[0] + variances[1]);
    assert!((result.inference().wald.statistic.unwrap() - independent_wald).abs() < 1e-8);
    assert_eq!(result.coefficients().kept_full_slots().len(), 2);
    assert_eq!(result.output().mean().len(), 10);
}

#[test]
fn final_sample_rebase_preserves_the_full_original_omission_axis() {
    let (request, loaded) = factor_rebase_fixture();
    let result = fit_prepared_ppml(request, loaded).expect("rebased factor composite PPML");
    let statuses = result.coefficients().statuses();
    assert_eq!(statuses[0], PpmlCoefficientStatus::BaseLevel);
    assert_eq!(statuses[3], PpmlCoefficientStatus::SyntaxOmitted);
    for &slot in &[1_usize, 2, 4] {
        assert!(matches!(
            statuses[slot],
            PpmlCoefficientStatus::Identified | PpmlCoefficientStatus::NumericallyRankDeficient
        ));
    }
    assert_eq!(result.coefficients().kept_full_slots().len(), 2);
    assert_eq!(result.coefficients().values().len(), 5);
    assert_eq!(result.inference().vce.covariance.len(), 25);
    for &slot in &[0_usize, 3] {
        assert_eq!(result.coefficients().values()[slot], 0.0);
        assert!(result.inference().vce.standard_errors[slot].is_nan());
    }
    assert_eq!(
        result.memory().plan.dimensions.reported_coefficient_slots,
        5
    );
    assert_eq!(result.memory().plan.dimensions.coefficient_slots, 5);
    let inference = result.inference();
    let retained_inference = (inference.vce.covariance.capacity()
        + inference.vce.standard_errors.capacity()
        + inference.vce.z_statistics.capacity()) as u128
        * std::mem::size_of::<f64>() as u128
        + inference.wald.tested_columns.capacity() as u128 * std::mem::size_of::<usize>() as u128;
    assert!(
        result
            .memory()
            .plan
            .selected_inventory()
            .inference_result_bytes
            >= retained_inference
    );
    assert!(result.certificate().passed);
}

#[test]
fn one_and_multiway_fe_use_normalized_constant_and_exact_final_topology() {
    for effects in [1, 2, 3] {
        let (request, loaded) = fixture(effects, false, false, false, false, false);
        let result = fit_prepared_ppml(request, loaded).expect("absorbed composite PPML");
        assert_eq!(result.fixed_effects().len(), effects);
        assert_eq!(
            result.coefficients().statuses().last(),
            Some(&PpmlCoefficientStatus::NormalizedStructuralConstant)
        );
        assert_eq!(
            result.inference().dof.finite_within_rank,
            result.inference().dof.sandwich_rank + 1
        );
        assert_eq!(
            result.inference().dof.absorbed_rank,
            result
                .fixed_effects()
                .iter()
                .map(|effect| effect.absorbed_rank)
                .sum()
        );
        assert!(
            result.convergence().inner_iterations
                > result.convergence().outer_iterations
                    + result.convergence().strict_profile_rejections
        );
        if effects == 3 {
            assert!(result.inference().dof.absorbed_rank_is_approximate);
            assert!(result
                .fixed_effects()
                .iter()
                .any(|effect| !effect.redundancy_exact));
        }
        assert!(result.output().certificate().passed);
    }
}

#[test]
fn ppml_result_freezes_pairwise_shared_topology_dof() {
    let (request, mut loaded) = fixture(3, false, false, false, false, false);
    let repeated_g = loaded
        .ols
        .numeric_sources
        .iter()
        .find(|(source, _)| source.get() == 2)
        .expect("first absorbed source")
        .1
        .clone();
    loaded
        .ols
        .numeric_sources
        .iter_mut()
        .find(|(source, _)| source.get() == 4)
        .expect("third absorbed source")
        .1 = repeated_g;

    let result = fit_prepared_ppml(request, loaded)
        .expect("pairwise-topology PPML result with a repeated third FE");
    let effect_ranks = result
        .fixed_effects()
        .iter()
        .map(|effect| effect.absorbed_rank)
        .collect::<Vec<_>>();

    // Pairwise mobility detects that the third partition repeats the
    // first. First-pair policy would leave five spurious absorbed
    // directions here after structural-constant normalization.
    assert_eq!(effect_ranks, [5, 1, 0]);
    assert_eq!(result.inference().dof.absorbed_rank, 6);
    assert!(!result.fixed_effects()[2].nested);
    assert!(!result.fixed_effects()[2].redundancy_exact);
    assert!(result.inference().dof.absorbed_rank_is_approximate);
    assert!(result.certificate().passed);
}

#[test]
fn allocation_audit_counts_only_executed_linear_subproblems() {
    let (request, loaded) = fixture(0, false, false, false, false, false);
    let normal = fit_prepared_ppml(request.clone(), loaded.clone()).expect("normal PPML");
    let mut audited_request = request;
    audited_request.allocation_snapshot_probe = Some(stable_allocation_probe);
    let audited = fit_prepared_ppml(audited_request, loaded).expect("audited PPML");
    // The audit deliberately defers strict convergence, but its start
    // depends on the first iteration whose complete shared-session reuse
    // report certifies.  Do not infer that warm-up boundary by subtracting
    // a separate normal fit.
    assert!(audited.convergence().outer_iterations > normal.convergence().outer_iterations);
    // One finite solve per outer iteration, plus the post-window
    // canonical-equivalence solve, final strict solve, and accepted-state
    // Hessian recertification. Any rejected strict solve adds one more
    // exact unit.
    assert_eq!(
        audited.convergence().inner_iterations,
        audited.convergence().outer_iterations
            + audited.convergence().strict_profile_rejections
            + 3
    );
    let certificate = audited
        .memory()
        .steady_state_allocation_certificate
        .as_ref()
        .expect("steady-state certificate");
    assert_eq!(
        certificate.observation.measured_iterations,
        PPML_STEADY_STATE_AUDIT_ITERATIONS
    );
}

#[test]
fn multipartition_ppml_map_keeps_fused_projection_capacities_stable() {
    for effects in [1, 2, 3] {
        let (mut request, loaded) = fixture(effects, false, false, false, false, false);
        request.allocation_snapshot_probe = Some(stable_allocation_probe);
        let result = fit_prepared_ppml(request, loaded).expect("audited categorical PPML");
        let certificate = result
            .memory()
            .steady_state_allocation_certificate
            .as_ref()
            .expect("categorical steady-state certificate");
        assert!(certificate.workspace_capacity_stable);
        assert!(certificate.linear_workspace_capacity_stable);
        assert!(certificate.observation.linear_session_allocation_free);
        assert!(certificate.observation.fresh_warm_equivalent);
        assert!(certificate.passed);
    }
}

#[test]
fn action_allocator_observation_is_attached_after_composite_certification() {
    let (mut request, loaded) = fixture(0, false, false, false, false, false);
    request.action_allocation_start = Some(PpmlAllocatorSnapshot {
        current_bytes: 512,
        peak_bytes: 768,
        allocation_events: 3,
    });
    request.action_allocation_snapshot_probe = Some(stable_allocation_probe);
    request.action_allocation_previous_baseline_bytes = Some(256);
    let result = fit_prepared_ppml(request, loaded).expect("observed PPML action");
    assert_eq!(result.memory().allocations.start_bytes, Some(512));
    assert_eq!(result.memory().allocations.current_bytes, Some(1_024));
    assert_eq!(result.memory().allocations.peak_bytes, Some(2_048));
    assert_eq!(
        result.memory().allocations.previous_baseline_bytes,
        Some(256)
    );
    assert!(result
        .memory()
        .steady_state_allocation_certificate
        .is_none());
}

#[test]
fn clustered_composite_uses_final_codes_and_shared_multiway_inventory() {
    let (request, loaded) = fixture(1, false, true, false, false, false);
    let result = fit_prepared_ppml(request, loaded).expect("clustered composite PPML");
    assert_eq!(result.clusters().len(), 1);
    assert_eq!(result.clusters()[0].declared_dimension, 0);
    assert_eq!(result.clusters()[0].groups, 4);
    assert_eq!(
        result.inference().vce.effective,
        super::super::PpmlVceKind::Clustered
    );
    assert_eq!(result.inference().vce.cluster_subsets.len(), 1);
    assert_eq!(result.inference().vce.cluster_subsets[0].groups, 4);
    assert!(result.inference().vce.certificate.passed);
}

#[test]
fn two_way_clustered_composite_publishes_every_final_subset() {
    let (mut request, mut loaded) = fixture(1, false, true, false, false, false);
    let second_cluster = SourceId::new(4).expect("second cluster source");
    request.ols.model.sources.push(source(second_cluster, "d"));
    request.ols.bindings.push(OlsInputBinding {
        stata_var_position: 6,
        role: OlsInputRole::NumericSource(second_cluster),
    });
    request.clusters.push(OlsClusterTermV1 {
        label: "d".to_owned(),
        sources: vec![second_cluster],
    });
    loaded.ols.numeric_sources.push((
        second_cluster,
        column(&[1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]),
    ));

    let result = fit_prepared_ppml(request, loaded).expect("two-way clustered PPML");
    assert_eq!(result.clusters().len(), 2);
    assert_eq!(
        result
            .clusters()
            .iter()
            .map(|cluster| cluster.groups)
            .collect::<Vec<_>>(),
        vec![4, 3]
    );
    let subsets = &result.inference().vce.cluster_subsets;
    assert_eq!(subsets.len(), 3);
    assert_eq!(
        subsets
            .iter()
            .map(|subset| (subset.mask, subset.dimensions.as_slice(), subset.sign))
            .collect::<Vec<_>>(),
        vec![(1, &[0][..], 1), (2, &[1][..], 1), (3, &[0, 1][..], -1)]
    );
    assert!(
        result.memory().plan.dimensions.cluster_subset_groups
            >= subsets.iter().map(|subset| subset.groups).sum()
    );
    assert!(result.inference().vce.certificate.passed);
}

#[test]
fn exact_fe_separation_rebases_every_final_product_once() {
    let (request, loaded) = fixture(1, false, false, false, false, true);
    let result = fit_prepared_ppml(request, loaded).expect("separated composite PPML");
    assert_eq!(
        result.sample().original_rows(),
        &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
    assert_eq!(
        &result.sample().drop_reasons()[..2],
        &[
            DropReason::FixedEffectSeparation,
            DropReason::FixedEffectSeparation,
        ]
    );
    assert_eq!(result.separation().separated_count, 2);
    assert_eq!(
        result.output().original_rows(),
        result.sample().original_rows()
    );
    assert_eq!(result.output().eta().len(), 10);
}

#[test]
fn requested_mu_without_a_candidate_is_reported_and_never_skipped() {
    let (mut request, mut loaded) = fixture(0, false, false, false, false, false);
    request.separation = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: true,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    loaded.ols.outcome.values[0] = 0.0;

    let result = fit_prepared_ppml(request, loaded).expect("Mu no-candidate PPML");
    assert_eq!(result.separation().generations, 0);
    let mu = result
        .separation()
        .methods
        .iter()
        .find(|report| report.method == SeparationMethod::Mu)
        .expect("requested Mu report");
    assert!(matches!(
        mu.status,
        MethodStatus::NoCandidates | MethodStatus::NoSeparation
    ));
    assert_ne!(mu.status, MethodStatus::SkippedUnsupported);
}

#[test]
fn default_auto_absorb_policy_is_resolved_once_for_separation_and_fit() {
    let (mut request, mut loaded) = fixture(0, false, false, false, false, false);
    request.separation = SeparationConfig::default();
    loaded.ols.outcome.values[0] = 0.0;

    let result = fit_prepared_ppml(request, loaded)
        .expect("default plugin-equivalent separation and fit policy");
    assert!(result.certificate().passed);
    assert!(result.separation().methods.iter().any(|report| {
        matches!(
            report.method,
            SeparationMethod::Simplex | SeparationMethod::IteratedRectifier
        )
    }));
}

#[test]
fn fitted_mu_drop_restarts_and_rebases_the_original_axis() {
    let (mut request, mut loaded) = fixture(1, false, false, false, false, false);
    // Keep the two-observation FE levels intact during the pre-fit chain.
    // The ordinary regressor then isolates row 1 exactly, so its fitted
    // zero-outcome mean approaches the lower boundary and the Mu
    // direction is an exact member of the prepared design span.
    request.ols.keep_singletons = true;
    request.separation = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: true,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    loaded.ols.outcome.values[1] = 0.0;
    let x = loaded
        .ols
        .numeric_sources
        .iter_mut()
        .find(|(source, _)| source.get() == 1)
        .expect("ordinary x source");
    x.1.values.fill(0.0);
    x.1.values[1] = -1.0;

    let result = fit_prepared_ppml(request, loaded).expect("Mu-restarted PPML");
    assert_eq!(result.separation().generations, 1);
    assert_eq!(result.separation().generation_hashes.len(), 2);
    assert_eq!(result.separation().separated_count, 1);
    assert_eq!(result.sample().drop_reasons()[1], DropReason::MuSeparation);
    assert!(!result.sample().original_rows().contains(&1));
    assert_eq!(
        result.output().original_rows(),
        result.sample().original_rows()
    );
    assert_eq!(
        result.separation().drop_reasons[1],
        Some(SeparationDropReason::Mu)
    );
    assert!(result.separation().methods.iter().any(|report| {
        report.method == SeparationMethod::Mu
            && report.status == MethodStatus::Certified
            && report.iterations > 0
            && report.dropped_observations == vec![1]
    }));
    assert!(result
        .separation()
        .methods
        .iter()
        .filter(|report| report.method == SeparationMethod::Mu)
        .all(|report| report.status != MethodStatus::SkippedUnsupported));
}

#[test]
fn accepted_mu_drop_then_publication_fit_failure_returns_no_composite() {
    let (mut request, mut loaded) = fixture(1, false, false, false, false, false);
    request.ols.keep_singletons = true;
    request.separation = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: true,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    loaded.ols.outcome.values[1] = 0.0;
    let x = loaded
        .ols
        .numeric_sources
        .iter_mut()
        .find(|(source, _)| source.get() == 1)
        .expect("ordinary x source");
    x.1.values.fill(0.0);
    x.1.values[1] = -1.0;

    // Reproduce the production preparation and one-time policy
    // resolution, but inject a failure at the only publication boundary.
    // The first solve is private Mu exploration; after its certified drop
    // the entire chain restarts, and no partially rebased result may
    // escape when the fresh final fit fails.
    request.ppml.absorb.retain_coefficients = false;
    if request.ppml.absorb.solver == FeSolverKind::Auto {
        request.ppml.absorb.solver = FeSolverKind::Map;
    }
    request.linear.fixed_effect_collinearity_tolerance =
        request.ppml.fixed_effect_collinearity_tolerance;
    request.linear.absorb = request.ppml.absorb;
    let numeric = prepare_numeric_input(&request, &loaded).expect("numeric PPML input");
    let dimensions = conservative_preflight_dimensions(&request, &numeric, 0)
        .expect("conservative preflight dimensions");
    let plan = plan_ppml_memory(dimensions, request.memory).expect("preflight plan");
    let mut resolved_linear = request.linear;
    resolved_linear.execution.mode = match plan.selected {
        PpmlMemoryRoute::Fast => crate::ols::PreparedOlsExecutionMode::Fast,
        PpmlMemoryRoute::Low => crate::ols::PreparedOlsExecutionMode::Low,
    };
    resolved_linear.execution.poolsize = request.memory.poolsize;
    let execution = PreparedPpmlSeparationExecution {
        preliminary_plan: plan,
        linear: resolved_linear,
    };
    let owner = PreparedPpmlModelOwner::new(request.ols, request.clusters, loaded.ols, numeric)
        .expect("prepared PPML owner");
    let ppml = request.ppml;
    let separation = request.separation;
    let mut exploratory_fits = 0_usize;
    let mut publication_fits = 0_usize;
    let full_axis_eta = [0.0, -30.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    let mut interrupt = |_| Ok(());
    let result = fit_with_prepared_ppml_separation(
        owner,
        ppml,
        separation,
        None,
        execution,
        &mut interrupt,
        |generation, _, _, _| {
            exploratory_fits += 1;
            let eta = generation
                .original_rows()
                .iter()
                .map(|row| full_axis_eta[*row])
                .collect();
            super::super::prepared_irls::PreparedPpmlMuExploration::from_test_eta(
                eta,
                generation,
                ppml.bounds,
            )
        },
        |_, _, _, _| {
            publication_fits += 1;
            Err::<(), _>(FeregError::new(
                ErrorCode::NonConvergence,
                "injected final PPML fit failure after accepted Mu drop",
            ))
        },
    );
    let error = match result {
        Ok(_) => panic!("a failed post-restart fit must publish no composite result"),
        Err(error) => error,
    };

    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert_eq!(exploratory_fits, 1);
    assert_eq!(publication_fits, 1);
}

#[test]
fn partial_saved_fe_publication_retains_full_final_hessian_certificate() {
    let (request, loaded) = fixture(2, true, false, false, false, false);
    let result = fit_prepared_ppml(request, loaded).expect("saved-FE composite PPML");
    let saved = result.output().saved_effects().expect("saved effects");
    assert_eq!(saved.declared_channels, 2);
    assert_eq!(saved.declared_channel_indices, vec![0]);
    assert_eq!(saved.channels.len(), 1);
    assert!(saved.full_decomposition_certified);
    assert_eq!(result.output().absorbed_predictor().len(), 12);
    assert!(saved.full_decomposition_relative_infinity_norm <= 1e-10);
    assert_eq!(result.fixed_effects()[0].saved_channel, Some(0));
    assert!(result.fixed_effects()[0].normalization_certified);
    assert_eq!(result.fixed_effects()[1].saved_channel, None);
    assert!(!result.fixed_effects()[1].normalization_applied);
}

#[test]
fn saved_effect_recovery_honors_admission_inside_a_different_outer_pool() {
    let outer = rayon::ThreadPoolBuilder::new()
        .num_threads(3)
        .build()
        .unwrap();
    for threads in [1, 2, 1] {
        let (mut request, loaded) = fixture(2, true, false, false, false, false);
        request.linear.execution.threads =
            crate::options::ThreadConfig::from_requested(threads, true).unwrap();
        let result = outer
            .install(|| fit_prepared_ppml(request, loaded))
            .expect("recovery must use the fit's admitted team, not the enclosing pool");
        assert_eq!(result.memory().effective_threads, threads);
        assert!(
            result
                .output()
                .saved_effects()
                .unwrap()
                .full_decomposition_certified
        );
        assert!(result.certificate().passed);
    }
}

#[test]
fn saved_intercept_and_slope_recover_through_ppml_fallible_map() {
    let (request, loaded) = saved_slope_fixture();

    let result = fit_prepared_ppml(request, loaded)
        .expect("saved PPML intercept and slope must use certified MAP recovery");
    let saved = result.output().saved_effects().expect("saved effects");
    assert_eq!(saved.declared_channels, 2);
    assert_eq!(saved.declared_channel_indices, [0, 1]);
    assert_eq!(saved.channels.len(), 2);
    assert!(saved.full_decomposition_certified);
    assert!(result.output().certificate().passed);
    assert!(result.certificate().passed);
    for row in 0..result.output().absorbed_predictor().len() {
        let sum = saved
            .channels
            .iter()
            .map(|channel| channel[row])
            .sum::<f64>();
        assert!((sum - result.output().absorbed_predictor()[row]).abs() <= 1e-10);
    }
    assert_eq!(result.fixed_effects().len(), 2);
    assert!(result
        .fixed_effects()
        .iter()
        .all(|effect| effect.normalization_certified && effect.reconstruction_certified));
}

#[test]
fn saved_slope_recovery_allocation_failure_is_r909_and_atomic() {
    let (mut request, loaded) = saved_slope_fixture();
    request.inject_fe_recovery_allocation_failure = true;

    let error = fit_prepared_ppml(request, loaded)
        .expect_err("injected saved-slope recovery allocation must publish no result");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(error.return_code(), 909);
    assert!(error.message.contains("complete FE recovery mask"));
}

#[test]
fn saved_fe_recovery_uses_the_hessian_of_the_published_strict_state() {
    // Mirrors the licensed-Stata reproducer that first exposed a stale
    // strict-frame handoff.  These integer outcomes do not lie exactly on
    // a log-linear surface, so the accepted strict update changes mu by a
    // small but scientifically relevant amount.  Requesting the saved FE
    // must still recover under q*mu for that same published state.
    let (request, mut loaded) = fixture(1, true, false, false, false, false);
    let rows = 80_usize;
    let x = (1..=rows)
        .map(|row| (row % 11) as f64 / 5.0 - 1.0)
        .collect::<Vec<_>>();
    let outcome = (1..=rows)
        .map(|row| {
            let base = (row % 5) as f64;
            if row % 13 == 0 {
                base + 1.0
            } else {
                base
            }
        })
        .collect::<Vec<_>>();
    let fixed_effect = (1..=rows)
        .map(|row| (1 + (row - 1) % 8) as f64)
        .collect::<Vec<_>>();
    loaded.ols.candidate = vec![true; rows].into();
    loaded.ols.outcome = column(&outcome);
    loaded.ols.numeric_sources[0].1 = column(&x);
    loaded.ols.numeric_sources[1].1 = column(&fixed_effect);

    let result = fit_prepared_ppml(request, loaded)
        .expect("saved FE must use the published state's exact final Hessian");
    let saved = result.output().saved_effects().expect("saved FE");
    assert!(saved.full_decomposition_certified);
    assert!(result.output().certificate().passed);
    assert!(result.certificate().passed);
}

#[test]
fn fit_only_and_residual_only_results_retain_no_unrequested_predictor_state() {
    let (mut fit_only_request, fit_only_loaded) = fixture(1, false, false, false, false, false);
    fit_only_request.output = PreparedPpmlOutputRequest {
        publication_matrix_cells: fit_only_request.output.publication_matrix_cells,
        ..PreparedPpmlOutputRequest::default()
    };
    let fit_only =
        fit_prepared_ppml(fit_only_request, fit_only_loaded).expect("fit-only PPML result");
    assert!(fit_only.output().xb().is_empty());
    assert!(fit_only.output().absorbed_predictor().is_empty());
    assert!(fit_only.output().eta().is_empty());
    assert!(fit_only.output().mean().is_empty());
    assert!(fit_only.output().saved_effects().is_none());
    assert_eq!(
        fit_only
            .memory()
            .plan
            .selected_inventory()
            .saved_output_bytes,
        0,
    );
    assert_eq!(
        fit_only
            .output()
            .retained_prediction(crate::ppml::PpmlPredictionKind::Eta)
            .expect_err("fit-only result must not retain eta")
            .code,
        ErrorCode::InvalidArgument,
    );
    assert!(fit_only.output().certificate().passed);
    assert!(fit_only.certificate().passed);

    let (mut residual_request, residual_loaded) = fixture(0, false, false, false, false, false);
    residual_request.output = PreparedPpmlOutputRequest {
        residual: Some(PpmlResidualKind::Response),
        requested_observation_columns: 1,
        publication_matrix_cells: residual_request.output.publication_matrix_cells,
        ..PreparedPpmlOutputRequest::default()
    };
    let residual =
        fit_prepared_ppml(residual_request, residual_loaded).expect("residual-only PPML result");
    assert!(residual.output().mean().is_empty());
    assert!(residual.output().eta().is_empty());
    assert_eq!(
        residual
            .output()
            .retained_prediction(crate::ppml::PpmlPredictionKind::Response)
            .expect("retained response residual")
            .len(),
        residual.sample().original_rows().len(),
    );
    assert_eq!(
        residual
            .memory()
            .plan
            .selected_inventory()
            .saved_output_bytes,
        residual.sample().original_rows().len() as u128 * size_of::<f64>() as u128,
    );
    assert!(residual.certificate().passed);
}

#[test]
fn pweights_and_offset_are_canonicalized_on_the_certified_final_sample() {
    let (request, loaded) = fixture(0, false, false, true, true, false);
    let result = fit_prepared_ppml(request, loaded).expect("weighted offset composite PPML");
    assert_eq!(result.weights().kind, Some(WeightKind::Probability));
    assert_eq!(
        result
            .memory()
            .plan
            .preflight_dimensions
            .retained_raw_preseparation_probability_weight_elements,
        12,
    );
    assert_eq!(
        result
            .memory()
            .plan
            .selected_inventory()
            .retained_raw_preseparation_probability_weight_bytes,
        12_u128 * size_of::<f64>() as u128,
    );
    assert!((result.weights().prepared_weight_sum - 12.0).abs() < 1e-12);
    assert_eq!(
        result.adjustment().input_kind,
        PreparedPpmlAdjustmentKind::Offset
    );
    assert_eq!(result.output().xb().len(), 12);
    assert!(result
        .output()
        .eta()
        .iter()
        .zip(result.output().xb())
        .zip(result.output().absorbed_predictor())
        .all(|((eta, xb), absorbed)| (eta - xb - absorbed).abs() <= 1e-10));
    assert!(result.output().certificate().passed);
}

#[test]
fn unrepresentable_pweight_range_returns_atomic_r430_with_and_without_fixed_effects() {
    for effects in [0_usize, 1] {
        let (request, mut loaded) = fixture(effects, true, false, true, false, false);
        let weights = &mut loaded
            .ols
            .probability_weight
            .as_mut()
            .expect("pweights")
            .values;
        weights.fill(1.0);
        // With one absorbed effect these are two internally equal FE
        // groups. Their local means are well-defined, but the common
        // canonical q vector spans more than one positive-f64 scale can
        // represent. The public policy therefore fails numerically
        // and publishes no shape-dependent partial result.
        weights[0] = f64::MAX;
        weights[1] = f64::MAX;
        weights[2] = f64::from_bits(1);
        weights[3] = f64::from_bits(1);
        let error = fit_prepared_ppml(request, loaded)
            .expect_err("unrepresentable normalized q must publish no composite result");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert_eq!(error.return_code(), 430);
        assert!(error.message.contains("computational scale"));
    }
}

#[test]
fn frequency_weights_and_exposure_reach_the_same_certified_boundary() {
    let (mut request, mut loaded) = fixture(0, false, false, false, false, false);
    request.ols.bindings.push(OlsInputBinding {
        stata_var_position: 4,
        role: OlsInputRole::FrequencyWeight,
    });
    loaded.ols.frequency_weight = Some(column(&[
        1.0, 2.0, 1.0, 3.0, 2.0, 1.0, 4.0, 1.0, 2.0, 3.0, 1.0, 2.0,
    ]));
    let exposure = [
        1.0_f64, 2.0, 0.5, 1.5, 3.0, 1.0, 2.5, 4.0, 1.25, 0.75, 2.0, 1.0,
    ];
    loaded.adjustment = Some(column(&exposure));
    loaded.adjustment_kind = PreparedPpmlAdjustmentKind::Exposure;

    let mut offset_loaded = loaded.clone();
    offset_loaded.adjustment = Some(column(&exposure.map(f64::ln)));
    offset_loaded.adjustment_kind = PreparedPpmlAdjustmentKind::Offset;
    let offset_result =
        fit_prepared_ppml(request.clone(), offset_loaded).expect("fweighted offset PPML");
    let result = fit_prepared_ppml(request, loaded).expect("fweighted exposure PPML");
    assert_eq!(result.weights().kind, Some(WeightKind::Frequency));
    assert_eq!(result.weights().effective_observations, 23);
    assert_eq!(result.weights().prepared_weight_sum, 23.0);
    assert_eq!(
        result.adjustment().input_kind,
        PreparedPpmlAdjustmentKind::Exposure
    );
    assert!(result.adjustment().prepared_as_offset);
    assert!(result.adjustment().exposure_log_transformed);
    assert_eq!(result.coefficients(), offset_result.coefficients());
    assert_eq!(result.inference(), offset_result.inference());
    assert_eq!(result.likelihood(), offset_result.likelihood());
    assert!(result.output().certificate().passed);
}

#[test]
fn canonical_profile_recovery_preserves_composite_publication_and_failure_atomicity() {
    for route in [MemoryMode::Fast, MemoryMode::Low] {
        for injections in [4, 100] {
            let (mut request, loaded) = fixture(0, false, false, false, false, false);
            request.memory.requested = route;
            request.memory.poolsize = 1;
            super::super::prepared_irls::inject_repeated_strict_resume(injections);
            let result = fit_prepared_ppml(request, loaded);
            super::super::prepared_irls::inject_repeated_strict_resume(0);
            if injections == 4 {
                let result =
                    result.expect("canonical recovery must publish a valid composite result");
                assert!(result.output().certificate().passed);
                assert_eq!(result.memory().fallback_count, 1);
            } else {
                let error = result.expect_err("exhausted recovery must return no composite result");
                assert_eq!(error.code, ErrorCode::NonConvergence);
                assert!(error.message.contains("canonical rebuild"));
            }
        }
    }
}

#[test]
fn domain_and_memory_failures_return_no_composite_value() {
    let (request, mut loaded) = fixture(0, false, false, false, false, false);
    loaded.ols.outcome.values[3] = -1.0;
    let error = fit_prepared_ppml(request, loaded).expect_err("negative outcome must fail");
    assert_eq!(error.code, ErrorCode::InvalidData);

    let (mut request, loaded) = fixture(0, false, false, false, false, false);
    request.memory.available_bytes = Some(1);
    request.phase_allocation_probe = Some(count_fit_phase_begins);
    FIT_PHASE_BEGINS.store(0, Ordering::SeqCst);
    let error = fit_prepared_ppml(request, loaded).expect_err("preflight must fail");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(
        FIT_PHASE_BEGINS.load(Ordering::SeqCst),
        0,
        "generation-zero preflight must fail before canonical preparation or any later phase",
    );

    let (mut request, loaded) = fixture(0, false, false, false, false, false);
    request.action_allocation_start = Some(PpmlAllocatorSnapshot::default());
    let error = fit_prepared_ppml(request, loaded)
        .expect_err("partial action allocation observation must fail atomically");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
}

#[test]
fn huge_finite_outcome_is_numerical_r430_with_no_composite_publication() {
    let (request, mut loaded) = fixture(0, false, false, false, false, false);
    loaded.ols.outcome.values.fill(1.0e307);
    let error = fit_prepared_ppml(request, loaded)
        .expect_err("a nonrepresentable finite-y fit must publish no composite result");
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert_eq!(error.return_code(), 430);
}

#[test]
fn bounded_integer_counts_match_exact_sets_and_reject_unbounded_domains() {
    use std::collections::BTreeSet;
    for shift in [-4_503_599_627_370_490_i64, -7, 0, 4_503_599_625_000_000_i64] {
        let values: Vec<f64> = [0, 2, 63, 64, 65, 2_097_151, 2_097_152, 2_097_153, 0, 64]
            .into_iter()
            .map(|value| (shift + value) as f64)
            .collect();
        let expected = values
            .iter()
            .map(|value| *value as i64)
            .collect::<BTreeSet<_>>()
            .len();
        for cap in [0, 1, 4, 100] {
            assert_eq!(
                bounded_integer_level_count(&values, cap, &mut |_| Ok(())).unwrap(),
                Some(expected.min(cap))
            );
        }
    }
    for values in [
        vec![0.5, 1.0],
        vec![0.0, f64::NAN],
        vec![0.0, f64::INFINITY],
        vec![
            0.0,
            (PREFLIGHT_INTEGER_MAX_PASSES * PREFLIGHT_INTEGER_WINDOW) as f64,
        ],
        vec![0.0, 4_503_599_627_370_497.0],
    ] {
        assert_eq!(
            bounded_integer_level_count(&values, 100, &mut |_| Ok(())).unwrap(),
            None
        );
    }
    assert_eq!(
        bounded_integer_level_count(&[-0.0, 0.0], 2, &mut |_| Ok(())).unwrap(),
        Some(1)
    );
    let mut polls = 0;
    let error = bounded_integer_level_count(&[0.0, 2_097_152.0], 10, &mut |_| {
        polls += 1;
        if polls == 3 {
            Err(FeregError::new(
                ErrorCode::Interrupted,
                "bitmap interrupted",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Interrupted);
}

#[test]
fn fe_run_bounds_dominate_exact_groups_under_order_masks_interactions_and_slopes() {
    use std::collections::BTreeSet;
    for case in 0..64_usize {
        let (mut request, mut loaded) = fixture(2, false, false, false, false, false);
        if case & 1 != 0 {
            let source = request.ols.absorbed_terms[1].categorical_sources[0];
            request.ols.absorbed_terms[0]
                .categorical_sources
                .push(source);
        }
        if case & 2 != 0 {
            request.ols.absorbed_terms[0]
                .slope_sources
                .push(SourceId::new(1).unwrap());
        }
        for (dimension, (_, column)) in loaded.ols.numeric_sources.iter_mut().enumerate().skip(1) {
            for (row, value) in column.values.iter_mut().enumerate() {
                *value = if case & 4 == 0 {
                    (row / (dimension + 1)) as f64
                } else {
                    ((row * (case + 3) + row * row * dimension) % 7) as f64
                };
            }
        }
        for mask in 0..16_usize {
            let active: Vec<_> = (0..12).filter(|row| ((row * 7 + mask) % 5) != 0).collect();
            let mut bound = conservative_preflight_dimensions_from_classification(
                &request,
                PpmlNumericClassification {
                    n_original: 12,
                    n_candidate: 12,
                    n_active: active.len(),
                },
                3,
                false,
                false,
                0,
            )
            .unwrap();
            refine_preflight_fe_bounds(&request.ols, &loaded.ols, &mut bound, &mut |_| Ok(()))
                .unwrap();
            let mut exact = bound;
            exact.fe_total_levels = 0;
            exact.fe_max_levels = 0;
            exact.fe_coefficient_directions = 0;
            exact.fe_max_effect_directions = 0;
            for term in &request.ols.absorbed_terms {
                let groups: BTreeSet<Vec<u64>> = active
                    .iter()
                    .map(|&row| {
                        term.categorical_sources
                            .iter()
                            .map(|source| {
                                loaded
                                    .ols
                                    .numeric_sources
                                    .iter()
                                    .find(|(id, _)| id == source)
                                    .unwrap()
                                    .1
                                    .values[row]
                                    .to_bits()
                            })
                            .collect()
                    })
                    .collect();
                let levels = groups.len();
                let directions = levels * (usize::from(term.intercept) + term.slope_sources.len());
                exact.fe_total_levels += levels;
                exact.fe_max_levels = exact.fe_max_levels.max(levels);
                exact.fe_coefficient_directions += directions;
                exact.fe_max_effect_directions = exact.fe_max_effect_directions.max(directions);
            }
            plan_ppml_memory(
                bound,
                PpmlMemoryConfig {
                    available_bytes: None,
                    ..request.memory
                },
            )
            .unwrap()
            .with_final_dimensions(exact)
            .unwrap();
            let upper = super::super::estimate_ppml_memory(bound, request.memory).unwrap();
            let actual = super::super::estimate_ppml_memory(exact, request.memory).unwrap();
            assert!(upper.fast.peak_bytes >= actual.fast.peak_bytes);
            assert!(upper.low.peak_bytes >= actual.low.peak_bytes);
        }
    }
}

#[test]
fn fe_run_refinement_is_interruptible_and_does_not_claim_unique_runs() {
    let (request, loaded) = fixture(2, false, false, false, false, false);
    let numeric = prepare_numeric_input(&request, &loaded).unwrap();
    let mut dimensions = conservative_preflight_dimensions(&request, &numeric, 0).unwrap();
    let original = dimensions;
    let error = refine_preflight_fe_bounds(&request.ols, &loaded.ols, &mut dimensions, &mut |_| {
        Err(FeregError::new(
            ErrorCode::Interrupted,
            "injected run-bound interrupt",
        ))
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert_eq!(dimensions, original);
    refine_preflight_fe_bounds(&request.ols, &loaded.ols, &mut dimensions, &mut |_| Ok(()))
        .unwrap();
    assert_eq!(dimensions.fe_total_levels, 18); // six ordered groups plus twelve alternating runs
    assert_eq!(dimensions.fe_max_levels, 12);
    assert_eq!(dimensions.fe_max_level_members, 12);
}

#[test]
fn cluster_preflight_bound_dominates_exact_subset_groups_without_n_storage() {
    let first = [0_u32, 0, 1, 1, 2, 2, 3, 3];
    let second = [0_u32, 1, 0, 1, 0, 1, 0, 1];
    let views = [first.as_slice(), second.as_slice()];
    let bound = cluster_subset_groups(&views).expect("cluster bound");
    let distinct = |rows: &mut dyn Iterator<Item = (u32, u32)>| {
        rows.collect::<std::collections::BTreeSet<_>>().len()
    };
    let exact = distinct(&mut first.iter().map(|&a| (a, 0)))
        + distinct(&mut second.iter().map(|&b| (b, 0)))
        + distinct(&mut first.iter().zip(&second).map(|(&a, &b)| (a, b)));
    assert!(bound >= exact);
}

fn fixture(
    effects: usize,
    save_first: bool,
    clustered: bool,
    probability_weighted: bool,
    offset: bool,
    separated: bool,
) -> (PreparedPpmlRequestV1, LoadedPpmlInput) {
    let rows = 12;
    let x = SourceId::new(1).expect("x");
    let mut sources = vec![source(x, "x")];
    let mut numeric_sources = vec![(
        x,
        column(&[
            -1.5, -0.5, -1.0, 0.5, -0.5, 1.0, 0.0, 1.5, 0.5, 2.0, 1.0, 2.5,
        ]),
    )];
    let mut absorbed_terms = Vec::new();
    if effects >= 1 {
        let g = SourceId::new(2).expect("g");
        sources.push(source(g, "g"));
        numeric_sources.push((
            g,
            column(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0, 6.0, 6.0]),
        ));
        absorbed_terms.push(absorbed(1, g, "g", if save_first { "alpha" } else { "" }));
    }
    if effects >= 2 {
        let h = SourceId::new(3).expect("h");
        sources.push(source(h, "h"));
        numeric_sources.push((
            h,
            column(&[1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0]),
        ));
        absorbed_terms.push(absorbed(2, h, "h", ""));
    }
    if effects >= 3 {
        let k = SourceId::new(4).expect("k");
        sources.push(source(k, "k"));
        numeric_sources.push((
            k,
            column(&[1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0]),
        ));
        absorbed_terms.push(absorbed(3, k, "k", ""));
    }
    if effects >= 4 {
        let m = SourceId::new(5).expect("m");
        sources.push(source(m, "m"));
        numeric_sources.push((
            m,
            column(&[1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]),
        ));
        absorbed_terms.push(absorbed(4, m, "m", ""));
    }
    let mut clusters = Vec::new();
    if clustered {
        let id = u32::try_from(sources.len() + 1).expect("cluster source id");
        let c = SourceId::new(id).expect("c");
        sources.push(source(c, "c"));
        numeric_sources.push((
            c,
            column(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 1.0, 2.0, 3.0, 4.0]),
        ));
        clusters.push(OlsClusterTermV1 {
            label: "c".to_owned(),
            sources: vec![c],
        });
    }
    let mut bindings = vec![
        OlsInputBinding {
            stata_var_position: 1,
            role: OlsInputRole::Candidate,
        },
        OlsInputBinding {
            stata_var_position: 2,
            role: OlsInputRole::Outcome,
        },
    ];
    for (index, (source, _)) in numeric_sources.iter().enumerate() {
        bindings.push(OlsInputBinding {
            stata_var_position: u32::try_from(index + 3).expect("source position"),
            role: OlsInputRole::NumericSource(*source),
        });
    }
    if probability_weighted {
        bindings.push(OlsInputBinding {
            stata_var_position: u32::try_from(numeric_sources.len() + 3).expect("weight position"),
            role: OlsInputRole::ProbabilityWeight,
        });
    }
    let ols = OlsRequestV1 {
        model: CompiledTermGraph {
            sources,
            terms: vec![continuous_term(x)],
        },
        absorbed_terms,
        bindings,
        include_intercept: true,
        keep_singletons: effects == 0,
    };
    let mut separation = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: separated,
            simplex: false,
            iterated_rectifier: false,
            mu: false,
        },
        ..SeparationConfig::default()
    };
    separation.drop_singletons = effects > 0;
    let ppml = PpmlConfig {
        max_iterations: 1_000,
        ..PpmlConfig::default()
    };
    let request = PreparedPpmlRequestV1 {
        bias: None,
        ols,
        clusters,
        vce: if clustered {
            PpmlVceRequest::Clustered
        } else {
            PpmlVceRequest::Robust
        },
        ppml,
        linear: PreparedOlsFitConfig::default(),
        memory: PpmlMemoryConfig::default(),
        memory_preflight_probe: None,
        allocation_snapshot_probe: None,
        phase_allocation_probe: None,
        linear_phase_allocation_probe: None,
        action_allocation_snapshot_probe: None,
        action_allocation_start: None,
        action_allocation_previous_baseline_bytes: None,
        separation,
        output: PreparedPpmlOutputRequest {
            residual: None,
            xb: true,
            absorbed: true,
            eta: true,
            mean: true,
            requested_observation_columns: 4 + usize::from(save_first && effects > 0),
            publication_matrix_cells: 512,
        },
        solver: PpmlSolverFamily::Map,
        inject_fe_recovery_allocation_failure: false,
    };
    let outcome = if separated {
        column(&[0.0, 0.0, 2.0, 4.0, 3.0, 5.0, 4.0, 7.0, 5.0, 8.0, 6.0, 9.0])
    } else {
        column(&[1.0, 2.0, 2.0, 4.0, 3.0, 5.0, 4.0, 7.0, 5.0, 8.0, 6.0, 9.0])
    };
    let probability_weight = probability_weighted
        .then(|| column(&[1.0, 2.0, 1.5, 0.5, 3.0, 2.0, 4.0, 1.0, 2.5, 3.5, 1.0, 2.0]));
    let adjustment = offset.then(|| {
        column(&[
            0.0, 0.05, 0.1, 0.0, -0.05, 0.1, 0.0, 0.05, -0.1, 0.0, 0.1, 0.05,
        ])
    });
    let loaded = LoadedPpmlInput {
        bias_units: None,
        ols: LoadedOlsInput {
            candidate: vec![true; rows].into(),
            outcome,
            numeric_sources,
            analytic_weight: None,
            frequency_weight: None,
            probability_weight,
        },
        adjustment,
        adjustment_kind: if offset {
            PreparedPpmlAdjustmentKind::Offset
        } else {
            PreparedPpmlAdjustmentKind::None
        },
    };
    (request, loaded)
}

#[test]
fn bias_exact_full_system_matches_independent_explicit_design() {
    let (mut request, mut loaded) = fixture(2, true, false, true, true, false);
    let ordinary = fit_prepared_ppml(request.clone(), loaded.clone()).unwrap();
    request.bias = Some(super::super::PpmlBiasRequest {
        method: super::super::PpmlBiasMethod::Exact,
        probes: 0,
        seed: 0,
        stream: 0,
        save_effects: true,
    });
    loaded.bias_units = Some(column(&[1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11., 12.]));
    let corrected = fit_prepared_ppml(request, loaded).unwrap();
    assert_eq!(ordinary.coefficients(), corrected.coefficients());
    assert_eq!(ordinary.output(), corrected.output());
    assert_eq!(
        ordinary.inference().vce.covariance,
        corrected.inference().vce.covariance
    );
    assert_eq!(ordinary.inference().dof, corrected.inference().dof);
    let bias = corrected.bias().unwrap();
    // Independently formed explicit X, J_g and K in ppml_bias_oracle.py.
    let raw = [
        -0.6300485701861334,
        0.30201008524001827,
        0.8998899114400561,
        1.4089593261745437,
        1.875388786600004,
        2.1284955000363808,
        0.,
        1.0631795606840344,
        -0.42003238012408883,
    ];
    let derivative = [
        -0.013673707941541657,
        -0.03641436939328413,
        -0.0033508271882589646,
        -0.004078391875716949,
        0.004277300708386278,
        0.014184178816249116,
        0.,
        0.009115805294373391,
        -0.009115805294346302,
    ];
    let curvature = [
        -0.0001930213526394886,
        0.006785593330410039,
        0.00038634085112329575,
        0.00034917161236207284,
        0.000364088184233044,
        0.00026489290492318074,
        0.,
        0.00012868090176021418,
        -0.0001286809017592705,
    ];
    for (got, expected) in [
        (&bias.raw, raw),
        (&bias.derivative, derivative),
        (&bias.curvature, curvature),
    ] {
        for (j, (&value, reference)) in got.iter().zip(expected).enumerate() {
            assert!(
                (value - reference).abs() < 2e-8,
                "coordinate {j}: {value} vs {reference}"
            );
        }
    }
    assert_eq!(bias.saved_effects.len(), 18);
    assert_eq!(bias.solves, 14);
    assert_eq!(bias.route, 2);
    assert!((0. ..=1.).contains(&bias.maximum_fe_leverage_lower_bound));
}

fn bias_fixture(randomized: bool) -> (PreparedPpmlRequestV1, LoadedPpmlInput) {
    let (mut request, mut loaded) = fixture(2, true, false, true, true, false);
    request.bias = Some(super::super::PpmlBiasRequest {
        method: if randomized {
            super::super::PpmlBiasMethod::Randomized
        } else {
            super::super::PpmlBiasMethod::Exact
        },
        probes: if randomized { 256 } else { 0 },
        seed: if randomized { 739391 } else { 0 },
        stream: if randomized { 7 } else { 0 },
        save_effects: true,
    });
    loaded.bias_units = Some(column(&[1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11., 12.]));
    (request, loaded)
}

#[test]
fn bias_weight_scale_low_memory_and_row_order() {
    let (request, loaded) = bias_fixture(true);
    let reference = fit_prepared_ppml(request.clone(), loaded.clone()).unwrap();
    for mode in 0..3 {
        let mut request = request.clone();
        let mut loaded = loaded.clone();
        if mode == 0 {
            for w in &mut loaded.ols.probability_weight.as_mut().unwrap().values {
                *w *= 17.;
            }
        } else if mode == 1 {
            request.memory.requested = MemoryMode::Low;
            request.memory.poolsize = 1;
        } else {
            loaded.ols.candidate.explicit_mut().unwrap().reverse();
            for col in std::iter::once(&mut loaded.ols.outcome)
                .chain(loaded.ols.numeric_sources.iter_mut().map(|(_, c)| c))
                .chain(loaded.ols.probability_weight.iter_mut())
                .chain(loaded.adjustment.iter_mut())
                .chain(loaded.bias_units.iter_mut())
            {
                col.values.reverse();
                col.missing.reverse();
            }
            request.linear.execution.threads =
                crate::ThreadConfig::from_requested(2, false).unwrap();
        }
        let result = fit_prepared_ppml(request, loaded).unwrap();
        assert_eq!(result.bias().unwrap().route, if mode == 1 { 1 } else { 2 });
        for (got, expected) in result
            .bias()
            .unwrap()
            .support
            .iter()
            .zip(&reference.bias().unwrap().support)
        {
            for (a, b) in got.values().into_iter().zip(expected.values()) {
                assert!((a - b).abs() < 1e-8, "support mode {mode}: {a} vs {b}");
            }
        }
        for (got, expected) in [
            &result.bias().unwrap().raw,
            &result.bias().unwrap().derivative,
            &result.bias().unwrap().curvature,
            &result.bias().unwrap().covariance,
        ]
        .iter()
        .zip([
            &reference.bias().unwrap().raw,
            &reference.bias().unwrap().derivative,
            &reference.bias().unwrap().curvature,
            &reference.bias().unwrap().covariance,
        ]) {
            for (&a, &b) in got.iter().zip(expected) {
                assert!((a - b).abs() < 2e-8, "mode {mode}: {a} vs {b}");
            }
        }
    }
}

#[test]
fn bias_rejects_fe_aligned_relabelled_and_partially_supported_units() {
    for randomized in [false, true] {
        for save_effects in [false, true] {
            for effect in [1, 2] {
                let (mut request, mut loaded) = bias_fixture(randomized);
                request.bias.as_mut().unwrap().save_effects = save_effects;
                loaded.bias_units.as_mut().unwrap().values = loaded.ols.numeric_sources[effect]
                    .1
                    .values
                    .iter()
                    .map(|x| 103. - 7. * x)
                    .collect();
                let e = fit_prepared_ppml(request, loaded).unwrap_err();
                assert!(
                    e.message
                        .contains("at least two distinct bias units per retained FE level"),
                    "{e}"
                );
            }
        }
    }
    let (request, mut loaded) = bias_fixture(false);
    loaded.bias_units.as_mut().unwrap().values[1] = 1.;
    let e = fit_prepared_ppml(request, loaded).unwrap_err();
    assert!(e.message.contains("FE1: 1/6 and FE2: 0/2"), "{e}");
    // A row restriction can create one-unit support even when the original
    // data passed. Retain singleton FEs so this tests the final-sample gate.
    let (mut request, mut loaded) = bias_fixture(false);
    request.ols.keep_singletons = true;
    request.separation.drop_singletons = false;
    loaded.ols.candidate.explicit_mut().unwrap()[0] = false;
    let e = fit_prepared_ppml(request, loaded).unwrap_err();
    assert!(e.message.contains("FE1: 1/6 and FE2: 0/2"), "{e}");
}

#[test]
fn bias_missing_units_fail_without_sample_selection() {
    let (request, mut loaded) = bias_fixture(false);
    loaded.bias_units.as_mut().unwrap().missing[3] = true;
    let error = fit_prepared_ppml(request, loaded).unwrap_err();
    assert!(
        error.to_string().contains("missing on a retained"),
        "{error}"
    );
}

#[test]
fn bias_rejects_separation_selected_sample_and_one_unit() {
    let (request, mut loaded) = bias_fixture(false);
    loaded.bias_units.as_mut().unwrap().values.fill(1.);
    let error = fit_prepared_ppml(request, loaded).unwrap_err();
    assert!(error.to_string().contains("at least two"), "{error}");
    let (mut request, mut loaded) = fixture(2, true, false, true, true, true);
    request.bias = bias_fixture(false).0.bias;
    loaded.bias_units = bias_fixture(false).1.bias_units;
    let error = fit_prepared_ppml(request, loaded).unwrap_err();
    assert!(
        error.to_string().contains("separation deletions"),
        "{error}"
    );
}

#[test]
fn bias_interrupt_before_auxiliary_solve_returns_no_result() {
    let (request, loaded) = bias_fixture(false);
    let mut seen = false;
    let result = fit_prepared_ppml_interruptible(request, loaded, &mut |point| {
        if point == PpmlInterruptPoint::BiasCorrection {
            seen = true;
            return Err(FeregError::new(
                ErrorCode::Interrupted,
                "bias test interruption",
            ));
        }
        Ok(())
    });
    assert!(seen);
    assert!(result.is_err());
}

#[test]
fn bias_randomized_full_system_expectation_and_variation() {
    let (request, loaded) = bias_fixture(false);
    let exact = fit_prepared_ppml(request, loaded.clone()).unwrap();
    let p = exact.bias().unwrap().raw.len();
    for probes in [8, 31, 256, 1024] {
        let mut total = vec![0.; p];
        let mut square = vec![0.; p];
        let mut variance = vec![0.; p];
        for seed in 0..64 {
            let mut request = bias_fixture(true).0;
            request.bias.as_mut().unwrap().seed = seed;
            request.bias.as_mut().unwrap().probes = probes;
            let result = fit_prepared_ppml(request, loaded.clone()).unwrap();
            let bias = result.bias().unwrap();
            for j in 0..p {
                let error = bias.value(j, 3).unwrap() - exact.bias().unwrap().value(j, 3).unwrap();
                total[j] += error;
                square[j] += error * error;
                variance[j] += bias.value(j, 8).unwrap().powi(2);
            }
        }
        for j in 0..p {
            if variance[j] > 1e-20 {
                assert!(
                    total[j].abs() < 4. * variance[j].sqrt(),
                    "coordinate {j}: numerical mean error"
                );
                assert!(
                    (square[j] / variance[j] - 1.).abs() < 0.6,
                    "coordinate {j}: numerical variance mismatch"
                );
            }
        }
    }
}

#[test]
fn bias_disconnected_components_anchor_and_reconstruct_on_both_routes() {
    let (request, mut loaded) = bias_fixture(false);
    loaded.ols.numeric_sources[2].1 = column(&[1., 2., 1., 2., 1., 2., 3., 4., 3., 4., 3., 4.]);
    for mode in [MemoryMode::Fast, MemoryMode::Low] {
        let mut request = request.clone();
        request.memory.requested = mode;
        let result = fit_prepared_ppml(request, loaded.clone()).unwrap();
        let bias = result.bias().unwrap();
        assert_eq!(bias.raw[6], 0.);
        assert_eq!(bias.raw[8], 0.);
        assert_eq!(bias.derivative[6], 0.);
        assert_eq!(bias.derivative[8], 0.);
        for row in 0..12 {
            let eta = bias.saved_effects[0][row]
                + bias.saved_effects[9][row]
                + bias.raw[10] * loaded.ols.numeric_sources[0].1.values[row]
                + loaded.adjustment.as_ref().unwrap().values[row];
            assert!((eta - result.output().mean()[row].ln()).abs() < 1e-8);
        }
    }
}

#[test]
fn bias_frequency_multiplicity_preserves_independent_units() {
    let (mut request, mut loaded) = bias_fixture(false);
    loaded.ols.probability_weight = None;
    loaded.ols.frequency_weight = Some(column(&[1., 2., 1., 3., 2., 1., 2., 3., 1., 2., 1., 2.]));
    for binding in &mut request.ols.bindings {
        if binding.role == OlsInputRole::ProbabilityWeight {
            binding.role = OlsInputRole::FrequencyWeight;
        }
    }
    let original = fit_prepared_ppml(request.clone(), loaded.clone()).unwrap();
    let multiplicity = loaded.ols.frequency_weight.take().unwrap().values;
    request
        .ols
        .bindings
        .retain(|b| b.role != OlsInputRole::FrequencyWeight);
    let repeat = |values: &[f64]| {
        values
            .iter()
            .zip(&multiplicity)
            .flat_map(|(&x, &m)| std::iter::repeat(x).take(m as usize))
            .collect::<Vec<_>>()
    };
    for col in std::iter::once(&mut loaded.ols.outcome)
        .chain(loaded.ols.numeric_sources.iter_mut().map(|(_, c)| c))
        .chain(loaded.adjustment.iter_mut())
        .chain(loaded.bias_units.iter_mut())
    {
        col.values = repeat(&col.values);
        col.missing = vec![false; col.values.len()];
    }
    loaded.ols.candidate = vec![true; loaded.ols.outcome.values.len()].into();
    let repeated = fit_prepared_ppml(request, loaded).unwrap();
    assert_eq!(
        original.bias().unwrap().units,
        repeated.bias().unwrap().units
    );
    for (got, expected) in repeated
        .bias()
        .unwrap()
        .support
        .iter()
        .zip(&original.bias().unwrap().support)
    {
        for (a, b) in got.values().into_iter().zip(expected.values()) {
            assert!((a - b).abs() < 1e-8, "frequency support: {a} vs {b}");
        }
    }
    for channel in [0, 1, 2, 3, 4] {
        for j in 0..original.bias().unwrap().raw.len() {
            assert!(
                (original.bias().unwrap().value(j, channel).unwrap()
                    - repeated.bias().unwrap().value(j, channel).unwrap())
                .abs()
                    < 1e-8
            );
        }
    }
}

fn make_finite_control_collinear_with_first_effect(loaded: &mut LoadedPpmlInput) {
    loaded.ols.numeric_sources[0].1 =
        column(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0, 6.0, 6.0]);
}

fn saved_slope_fixture() -> (PreparedPpmlRequestV1, LoadedPpmlInput) {
    let (mut request, mut loaded) = fixture(1, true, false, false, false, false);
    let slope = SourceId::new(1).expect("slope source");
    request.ols.absorbed_terms[0].slope_sources.push(slope);
    request.output.requested_observation_columns = 6;
    loaded.ols.numeric_sources[1].1 =
        column(&[1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0]);
    (request, loaded)
}

fn stable_allocation_probe() -> PpmlAllocatorSnapshot {
    PpmlAllocatorSnapshot {
        current_bytes: 1_024,
        peak_bytes: 2_048,
        allocation_events: 17,
    }
}

fn factor_rebase_fixture() -> (PreparedPpmlRequestV1, LoadedPpmlInput) {
    let rows = 12;
    let g = SourceId::new(1).expect("g");
    let x = SourceId::new(2).expect("x");
    let ols = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![source(g, "g"), source(x, "x")],
            terms: vec![
                categorical_term(1, g, 1.0, true),
                categorical_term(2, g, 2.0, false),
                categorical_term(3, g, 3.0, false),
                omitted_continuous_term(4, x),
            ],
        },
        absorbed_terms: Vec::new(),
        bindings: vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::NumericSource(g),
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::NumericSource(x),
            },
        ],
        include_intercept: true,
        keep_singletons: true,
    };
    let mut outcome = column(&[1.0, 2.0, 1.0, 2.0, 1.0, 3.0, 2.0, 3.0, 4.0, 5.0, 4.0, 6.0]);
    outcome.missing[0] = true;
    outcome.missing[1] = true;
    let loaded = LoadedPpmlInput {
        bias_units: None,
        ols: LoadedOlsInput {
            candidate: vec![true; rows].into(),
            outcome,
            numeric_sources: vec![
                (
                    g,
                    column(&[1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0, 3.0]),
                ),
                (
                    x,
                    column(&[
                        -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, -1.5, -0.5, 0.5, 1.5, 2.5,
                    ]),
                ),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        },
        adjustment: None,
        adjustment_kind: PreparedPpmlAdjustmentKind::None,
    };
    let ppml = PpmlConfig {
        max_iterations: 1_000,
        ..PpmlConfig::default()
    };
    (
        PreparedPpmlRequestV1 {
            bias: None,
            ols,
            clusters: Vec::new(),
            vce: PpmlVceRequest::Robust,
            ppml,
            linear: PreparedOlsFitConfig::default(),
            memory: PpmlMemoryConfig::default(),
            memory_preflight_probe: None,
            allocation_snapshot_probe: None,
            phase_allocation_probe: None,
            linear_phase_allocation_probe: None,
            action_allocation_snapshot_probe: None,
            action_allocation_start: None,
            action_allocation_previous_baseline_bytes: None,
            separation: SeparationConfig::default(),
            output: PreparedPpmlOutputRequest {
                residual: None,
                xb: false,
                absorbed: false,
                eta: false,
                mean: false,
                requested_observation_columns: 0,
                publication_matrix_cells: 512,
            },
            solver: PpmlSolverFamily::Auto,
            inject_fe_recovery_allocation_failure: false,
        },
        loaded,
    )
}

fn source(id: SourceId, label: &str) -> SourceEntry {
    SourceEntry {
        id,
        canonical_name: label.to_owned(),
    }
}

fn absorbed(id: u32, source: SourceId, label: &str, saved: &str) -> AbsorbedTerm {
    AbsorbedTerm {
        id: AbsorbTermId::new(id).expect("absorb id"),
        intercept: true,
        categorical_sources: vec![source],
        slope_sources: Vec::new(),
        label: label.to_owned(),
        saved_name: saved.to_owned(),
    }
}

fn continuous_term(source: SourceId) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(1).expect("term"),
        order: TermOrder::new(1).expect("term order"),
        coefficient_slot: CoefficientSlot::new(1).expect("coefficient slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: "c.x".to_owned(),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component order"),
            source,
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.0,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: "c".to_owned(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn omitted_continuous_term(id: u32, source: SourceId) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(id).expect("term"),
        order: TermOrder::new(id).expect("term order"),
        coefficient_slot: CoefficientSlot::new(id).expect("coefficient slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: true,
        canonical_token: "o.x".to_owned(),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component order"),
            source,
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.0,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: "c".to_owned(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn categorical_term(id: u32, source: SourceId, level: f64, base: bool) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(id).expect("term"),
        order: TermOrder::new(id).expect("term order"),
        coefficient_slot: CoefficientSlot::new(id).expect("coefficient slot"),
        raw_kind: RawTermKind::Factor,
        whole_column_omitted: base,
        canonical_token: format!("{level}.g"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component order"),
            source,
            kind: ComponentKind::Categorical,
            level_state: LevelState::Present,
            level_value: level,
            base: if base {
                MetadataFlag::True
            } else {
                MetadataFlag::False
            },
            omitted: if base {
                MetadataFlag::True
            } else {
                MetadataFlag::False
            },
            raw_operator: level.to_string(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn column(values: &[f64]) -> LoadedNumericColumn {
    LoadedNumericColumn {
        values: values.to_vec(),
        missing: vec![false; values.len()],
    }
}

#[test]
fn candidate_uniform_raw_owner_memory_counts_actual_explicit_capacity() {
    use crate::data::UniformValues;
    let (request, mut loaded) = fixture(1, false, false, false, false, false);
    let rows = loaded.ols.candidate.len();
    loaded.ols.candidate = UniformValues::Repeated {
        len: rows,
        value: true,
    };
    let implicit = raw_input_owner_bytes(&request, &loaded);
    let mut explicit = Vec::with_capacity(rows + 73);
    explicit.resize(rows, true);
    let capacity = explicit.capacity() as u128;
    loaded.ols.candidate = explicit.into();
    assert_eq!(
        raw_input_owner_bytes(&request, &loaded),
        implicit + capacity
    );
}
