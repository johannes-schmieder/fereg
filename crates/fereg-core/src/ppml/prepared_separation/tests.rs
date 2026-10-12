use super::*;

/// Explicit fitted start for focused Mu detector tests (original axis).
#[derive(Debug, Clone, PartialEq)]
struct PpmlStart {
    coefficients: Vec<f64>,
    /// Includes the offset.
    linear_predictor: Vec<f64>,
}

/// Separation-only adapter retained for focused detector tests. Production
/// uses `fit_with_prepared_ppml_separation`, so a requested Mu method without
/// an explicit fitted start fails instead of silently publishing a skip.
fn detect_prepared_ppml_separation(
    owner: PreparedPpmlModelOwner,
    ppml: PpmlConfig,
    config: SeparationConfig,
    phase_allocation_probe: Option<PpmlPhaseAllocatorProbe>,
    start: Option<&PpmlStart>,
) -> Result<PreparedPpmlSeparationOutcome> {
    let supplied_start = start.cloned();
    validate_config(config)?;
    let coordinator = PreparedSeparationCoordinator::new(owner)?;
    let linear = PreparedOlsFitConfig {
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    let mut interrupt = |_| Ok(());
    let (outcome, ()) = coordinate_prepared_ppml_separation(
        coordinator,
        ppml,
        config,
        phase_allocation_probe,
        linear,
        ppml.absorb.solver_memory_budget.is_some(),
        &mut interrupt,
        |generation, _| {
            let fitted_start = match &supplied_start {
                Some(start) => start,
                None if config.methods.mu => {
                    return Err(FeregError::new(
                        ErrorCode::Unsupported,
                        "requested prepared Mu separation requires a fitted generation start",
                    ));
                }
                None => unreachable!("Mu exploration is disabled"),
            };
            if fitted_start.linear_predictor.len() != generation.original_observations() {
                return Err(invalid_argument(
                    "prepared Mu test start does not match the original axis",
                ));
            }
            let eta = generation
                .original_rows()
                .iter()
                .map(|row| fitted_start.linear_predictor[*row])
                .collect();
            PreparedPpmlMuExploration::from_test_eta(eta, generation, ppml.bounds)
        },
        |_, _| Ok(()),
    )?;
    Ok(outcome)
}

use crate::data::{NumericColumnView, NumericSourceUse, WeightInput};
use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder,
    ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent,
    TermId, TermOrder, TimeSeriesState,
};
use crate::ols::{
    LoadedNumericColumn, LoadedOlsInput, OlsClusterTermV1, OlsInputBinding, OlsInputRole,
    OlsRequestV1,
};
use crate::ppml::prepared::{prepare_ppml_numeric_data, PpmlNumericPreparationInput};
use crate::ppml::PreparedPpmlNumericData;
use crate::ppml_separation::SeparationMethods;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

static PHASE_PROBE_TEST_LOCK: Mutex<()> = Mutex::new(());
static PHASE_PROBE_EVENTS: Mutex<Vec<(PpmlPhase, super::super::PpmlPhaseAllocatorEvent, u64)>> =
    Mutex::new(Vec::new());
static PHASE_PROBE_ENDS: AtomicU64 = AtomicU64::new(0);
static MU_PHASE_ACTIVE: AtomicBool = AtomicBool::new(false);
static MU_FIT_CALLS: AtomicU64 = AtomicU64::new(0);

#[test]
fn scheduled_seals_keep_byte_identity_and_reject_mutated_bindings() {
    let n = 8193;
    let rows: Vec<_> = (0..n).map(|i| 5 + 3 * i).collect();
    let y: Vec<_> = (0..n).map(|i| [0., 0.25, 2., 1e100][i % 4]).collect();
    let report = SeparationReport {
        candidate_observations: n,
        active: vec![true; n],
        retained_observations: (0..n).collect(),
        dropped_observations: vec![],
        drop_reasons: vec![None; n],
        singleton_count: 0,
        separated_count: 0,
        generations: 0,
        generation_hashes: vec![candidate_ledger_hash_candidate_axis(
            &rows,
            &vec![true; n],
            &vec![None; n],
        )
        .unwrap()],
        methods: vec![],
    };
    let config = SeparationConfig {
        drop_singletons: false,
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: false,
        },
        ..SeparationConfig::default()
    };
    let scalar = certify_disabled_separation_for_test(config, &report, &rows, &y).unwrap();
    for threads in [1, 2, 4, 8] {
        crate::parallel::ThreadTeam::new(
            crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
        )
        .install(|| {
            let actual = certify_disabled_separation_for_test(config, &report, &rows, &y).unwrap();
            assert_eq!(actual, scalar);
            actual.validate(config, &report, &rows, &y).unwrap();
            let mut changed_rows = rows.clone();
            changed_rows[4000] += 1;
            let mut changed_y = y.clone();
            changed_y[4000] = f64::NAN;
            let mut changed_report = report.clone();
            changed_report.active[4000] = false;
            for (r, o, t) in [
                (&changed_rows, &y, &report),
                (&rows, &changed_y, &report),
                (&rows, &y, &changed_report),
                (&changed_rows, &changed_y, &changed_report),
            ] {
                let expected = std::thread::scope(|scope| {
                    scope
                        .spawn(|| scalar.validate(config, t, r, o))
                        .join()
                        .unwrap()
                })
                .unwrap_err();
                let found = actual.validate(config, t, r, o).unwrap_err();
                assert_eq!(
                    (found.code, found.message),
                    (expected.code, expected.message)
                );
            }
        })
        .unwrap();
    }
}

#[test]
fn initial_generation_phase_closes_on_success_and_capacity_rejection() {
    let _guard = PHASE_PROBE_TEST_LOCK.lock().unwrap();
    for rejected in [false, true] {
        PHASE_PROBE_EVENTS.lock().unwrap().clear();
        PHASE_PROBE_ENDS.store(0, Ordering::SeqCst);
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let mut owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).unwrap();
        if rejected {
            owner = owner.with_generation_memory_bound(0);
        }
        let result = prepare_initial_generation(owner, Some(deterministic_phase_probe));
        if rejected {
            assert_eq!(result.err().unwrap().code, ErrorCode::Allocation);
        } else {
            assert_eq!(result.unwrap().2, Some(1));
        }
        assert_eq!(
            *PHASE_PROBE_EVENTS.lock().unwrap(),
            vec![
                (
                    PpmlPhase::Preparation,
                    super::super::PpmlPhaseAllocatorEvent::Begin,
                    0
                ),
                (
                    PpmlPhase::Preparation,
                    super::super::PpmlPhaseAllocatorEvent::End,
                    1
                ),
            ]
        );
    }
}

#[test]
fn impossible_detector_workspace_reservation_maps_to_allocation() {
    let error = fallible_collect(
        std::iter::empty::<usize>(),
        usize::MAX,
        "injected prepared separation allocation failure",
    )
    .expect_err("an impossible data-sized reservation must fail");
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(error.return_code(), 909);
}

#[test]
fn report_bank_overflow_or_allocation_failure_precedes_detector_execution() {
    assert_eq!(
        checked_method_report_capacity(usize::MAX, 2)
            .expect_err("method bank overflow")
            .code,
        ErrorCode::Allocation,
    );

    let _guard = PHASE_PROBE_TEST_LOCK.lock().expect("phase-probe lock");
    PHASE_PROBE_EVENTS
        .lock()
        .expect("phase-probe events")
        .clear();
    PHASE_PROBE_ENDS.store(0, Ordering::SeqCst);
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    FAIL_REPORT_BANK_RESERVATION.set(true);
    let error = match detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        },
        Some(deterministic_phase_probe),
        None,
    ) {
        Ok(_) => panic!("injected report-bank reservation must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(
        PHASE_PROBE_EVENTS
            .lock()
            .expect("phase-probe events")
            .is_empty(),
        "no detector phase may begin before both bounded report banks reserve",
    );
}

fn deterministic_phase_probe(
    phase: PpmlPhase,
    event: super::super::PpmlPhaseAllocatorEvent,
) -> u64 {
    let value = match event {
        super::super::PpmlPhaseAllocatorEvent::Begin => 0,
        super::super::PpmlPhaseAllocatorEvent::End => {
            PHASE_PROBE_ENDS.fetch_add(1, Ordering::SeqCst) + 1
        }
    };
    PHASE_PROBE_EVENTS
        .lock()
        .expect("phase-probe events")
        .push((phase, event, value));
    value
}

fn mu_scope_probe(phase: PpmlPhase, event: super::super::PpmlPhaseAllocatorEvent) -> u64 {
    if phase == PpmlPhase::SeparationMu {
        MU_PHASE_ACTIVE.store(
            event == super::super::PpmlPhaseAllocatorEvent::Begin,
            Ordering::SeqCst,
        );
    }
    0
}

#[test]
fn phase_probe_brackets_disabled_stages_and_max_merges_restarts() {
    let _guard = PHASE_PROBE_TEST_LOCK.lock().expect("phase-probe lock");
    PHASE_PROBE_ENDS.store(0, Ordering::SeqCst);
    PHASE_PROBE_EVENTS
        .lock()
        .expect("phase-probe events")
        .clear();
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let result = detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: true,
                simplex: true,
                iterated_rectifier: true,
                mu: false,
            },
            drop_singletons: true,
            ..SeparationConfig::default()
        },
        Some(deterministic_phase_probe),
        None,
    )
    .expect("phase-observed separation");
    let events = PHASE_PROBE_EVENTS.lock().expect("phase-probe events");
    for phase in SEPARATION_PHASES {
        let begins = events
            .iter()
            .filter(|(observed, event, _)| {
                *observed == phase && *event == super::super::PpmlPhaseAllocatorEvent::Begin
            })
            .count();
        let ends = events
            .iter()
            .filter(|(observed, event, _)| {
                *observed == phase && *event == super::super::PpmlPhaseAllocatorEvent::End
            })
            .count();
        assert!(begins > 0, "missing phase boundary for {phase:?}");
        assert_eq!(begins, ends, "unbalanced phase boundary for {phase:?}");
        let expected_peak = events
            .iter()
            .filter_map(|(observed, event, value)| {
                (*observed == phase && *event == super::super::PpmlPhaseAllocatorEvent::End)
                    .then_some(*value)
            })
            .max();
        assert_eq!(result.phase_allocation_peak(phase), expected_peak);
    }
    assert!(
        events
            .iter()
            .filter(|(phase, event, _)| {
                *phase == PpmlPhase::SeparationFe
                    && *event == super::super::PpmlPhaseAllocatorEvent::End
            })
            .count()
            > 1,
        "the fixture must exercise max-merging across an FE restart"
    );
}

#[test]
fn mu_phase_and_report_charge_the_fitted_start_solve() {
    let _guard = PHASE_PROBE_TEST_LOCK.lock().expect("phase-probe lock");
    MU_PHASE_ACTIVE.store(false, Ordering::SeqCst);
    MU_FIT_CALLS.store(0, Ordering::SeqCst);
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
    let ppml = PpmlConfig::default();
    let linear = PreparedOlsFitConfig {
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    let mut interrupt = |_| Ok(());
    let (result, artifact) = coordinate_prepared_ppml_separation(
        coordinator,
        ppml,
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: true,
                mu: true,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        },
        Some(mu_scope_probe),
        linear,
        false,
        &mut interrupt,
        |generation, _| {
            assert!(
                MU_PHASE_ACTIVE.load(Ordering::SeqCst),
                "the fitted Mu start must be solved inside the Mu phase"
            );
            MU_FIT_CALLS.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(5));
            PreparedPpmlMuExploration::from_test_eta(
                vec![0.0; generation.original_rows().len()],
                generation,
                ppml.bounds,
            )
        },
        |_, _| Ok(17_u8),
    )
    .expect("Mu-coordinated fit");

    assert_eq!(artifact, 17);
    assert_eq!(MU_FIT_CALLS.load(Ordering::SeqCst), 1);
    assert!(!MU_PHASE_ACTIVE.load(Ordering::SeqCst));
    let mu = result
        .separation
        .methods
        .iter()
        .find(|report| report.method == SeparationMethod::Mu)
        .expect("requested Mu report");
    assert_eq!(mu.status, MethodStatus::NoCandidates);
    assert_eq!(mu.iterations, 0);
    assert!(mu.certificate.is_none());
    assert_ne!(mu.status, MethodStatus::SkippedUnsupported);
    assert!(
        mu.elapsed_seconds >= 0.004,
        "Mu report excluded its fitted-start solve: {} seconds",
        mu.elapsed_seconds
    );
}

#[test]
fn candidate_ledger_hash_binds_stable_original_rows() {
    let active = [false, true, true, true, true];
    let reasons = [None; 5];
    let first = candidate_ledger_hash_original_axis(&[1, 2], &active, &reasons)
        .expect("first candidate ledger hash");
    let second = candidate_ledger_hash_original_axis(&[3, 4], &active, &reasons)
        .expect("second candidate ledger hash");
    assert_ne!(first, second);

    let compact_active = [true, true];
    let compact_reasons = [None, None];
    assert_eq!(
        first,
        candidate_ledger_hash_candidate_axis(&[1, 2], &compact_active, &compact_reasons)
            .expect("compact replay hash")
    );
}

#[test]
fn ir_no_separation_boundary_diagnostic_is_never_negative() {
    assert_eq!(nonnegative_ir_boundary_violation(0.25), 0.0);
    assert_eq!(nonnegative_ir_boundary_violation(0.0), 0.0);
    assert_eq!(nonnegative_ir_boundary_violation(-0.25), 0.25);
}

#[test]
fn invalid_detector_tolerance_fails_closed_before_preparation() {
    let error = validate_config(SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: true,
        },
        mu_tolerance: f64::NAN,
        ..SeparationConfig::default()
    })
    .expect_err("nonfinite mu tolerance must fail closed");
    assert_eq!(error.code, ErrorCode::InvalidArgument);

    let error = validate_config(SeparationConfig {
        tolerance: 1.0,
        ..SeparationConfig::default()
    })
    .expect_err("unit separation tolerance must fail closed");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
}

#[test]
fn numerical_proposal_boundary_rejects_every_identity_and_shape_forgery() {
    let (request, clusters, mut loaded, uses) = fixture();
    // Original row zero remains a real source observation but is inactive
    // in generation zero, which distinguishes inactive from out-of-range.
    loaded.candidate.explicit_mut().unwrap()[0] = false;
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
    let generation = coordinator.view().prepared;
    assert_eq!(generation.original_rows(), &[1, 2, 3, 4, 5]);
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: false,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let claimed = SeparationCertificate {
        proof: crate::ppml_separation::SeparationCertificateProof::RecessionDirection,
        interior_zero_relative: 0.0,
        boundary_violation_relative: 0.0,
        span_residual_relative: 0.0,
        negative_margin_relative: 1.0,
        direction_scale: 1.0,
        positive_maximum: 0.0,
        passed: true,
    };
    let raw = |dropped: Vec<usize>| DetectionProposal {
        method: SeparationMethod::Simplex,
        dropped,
        iterations: 1,
        status: MethodStatus::Certified,
        certificate: Some(claimed.clone()),
        exact_fe_certificate: None,
        direction: Some(vec![-1.0; generation.original_rows().len()]),
        note: "raw adversarial proposal".into(),
    };
    let descending = bind_numerical_proposal(generation, raw(vec![2, 0]))
        .expect("binding must preserve a descending detector result");
    assert_eq!(descending.dropped_original_rows, vec![3, 1]);
    assert!(validate_prepared_numerical_metadata(generation, &descending, config).is_err());
    let duplicate = bind_numerical_proposal(generation, raw(vec![0, 0]))
        .expect("binding must preserve duplicate detector rows");
    assert_eq!(duplicate.dropped_original_rows, vec![1, 1]);
    assert!(validate_prepared_numerical_metadata(generation, &duplicate, config).is_err());
    assert!(
        bind_numerical_proposal(generation, raw(vec![generation.original_rows().len()]),).is_err()
    );
    let base = PreparedNumericalProposal {
        generation: generation.generation(),
        generation_rows_digest: stable_usize_slice_digest(
            b"numerical-proposal-rows",
            generation.original_rows(),
        )
        .expect("row binding"),
        generation_outcome_digest: stable_f64_slice_digest(
            b"numerical-proposal-outcome",
            generation.outcome(),
        )
        .expect("outcome binding"),
        method: SeparationMethod::Simplex,
        dropped_original_rows: vec![1],
        iterations: 1,
        status: MethodStatus::Certified,
        certificate: Some(claimed),
        exact_fe_certificate: None,
        direction: Some(vec![-1.0; generation.original_rows().len()]),
        exact_intercept_span: None,
        note: "adversarial proposal".into(),
        fixed_point: None,
    };

    let mut forgeries = Vec::new();
    let mut unsorted = base.clone();
    unsorted.dropped_original_rows = vec![3, 1];
    forgeries.push(unsorted);
    let mut duplicate = base.clone();
    duplicate.dropped_original_rows = vec![1, 1];
    forgeries.push(duplicate);
    let mut inactive = base.clone();
    inactive.dropped_original_rows = vec![0];
    forgeries.push(inactive);
    let mut out_of_range = base.clone();
    out_of_range.dropped_original_rows = vec![usize::MAX];
    forgeries.push(out_of_range);
    let mut stale = base.clone();
    stale.generation += 1;
    forgeries.push(stale);
    let mut wrong_generation_binding = base.clone();
    wrong_generation_binding.generation_outcome_digest ^= 1;
    forgeries.push(wrong_generation_binding);
    let mut wrong_method = base.clone();
    wrong_method.method = SeparationMethod::FixedEffects;
    forgeries.push(wrong_method);
    let mut wrong_status = base.clone();
    wrong_status.status = MethodStatus::NoSeparation;
    forgeries.push(wrong_status);
    let mut wrong_certificate = base.clone();
    wrong_certificate
        .certificate
        .as_mut()
        .expect("certificate")
        .proof =
        crate::ppml_separation::SeparationCertificateProof::SimplexFullPositiveRank { columns: 1 };
    forgeries.push(wrong_certificate);

    for forged in forgeries {
        assert!(
            validate_prepared_numerical_metadata(generation, &forged, config).is_err(),
            "forged proposal crossed the prepared numerical boundary: {forged:?}"
        );
    }

    let mu_config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: true,
        },
        drop_singletons: false,
        ..config
    };
    let mut mu_no_candidates = base.clone();
    mu_no_candidates.method = SeparationMethod::Mu;
    mu_no_candidates.dropped_original_rows.clear();
    mu_no_candidates.iterations = 0;
    mu_no_candidates.status = MethodStatus::NoCandidates;
    mu_no_candidates.certificate = None;
    mu_no_candidates.direction = None;
    validate_prepared_numerical_metadata(generation, &mu_no_candidates, mu_config)
        .expect("Mu has one intentional proof-free no-drop grammar");

    let mut false_mu_no_separation = mu_no_candidates.clone();
    false_mu_no_separation.iterations = 1;
    false_mu_no_separation.status = MethodStatus::NoSeparation;
    false_mu_no_separation.certificate = base.certificate.clone();
    assert!(
        validate_prepared_numerical_metadata(generation, &false_mu_no_separation, mu_config,)
            .is_err()
    );

    let mut polluted_mu_no_candidates = mu_no_candidates;
    polluted_mu_no_candidates.certificate = base.certificate.clone();
    assert!(validate_prepared_numerical_metadata(
        generation,
        &polluted_mu_no_candidates,
        mu_config,
    )
    .is_err());
    assert_eq!(coordinator.view().generation, 0);
    assert_eq!(coordinator.view().original_rows, &[1, 2, 3, 4, 5]);
    assert!(coordinator.reasons().iter().all(Option::is_none));
}

#[test]
fn independent_certificate_and_commit_token_are_both_required_atomically() {
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let mut coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
    let generation = coordinator.view().prepared;
    let ppml = PpmlConfig::default();
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: false,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let linear = PreparedOlsFitConfig {
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    let mut interrupt = |_| Ok(());
    let proposal = bind_numerical_proposal(
        generation,
        prepared_simplex_proposal(generation, config, &mut interrupt).expect("simplex proposal"),
    )
    .expect("stable proposal");
    assert!(!proposal.dropped_original_rows.is_empty());
    let validated = validate_numerical_proposal(generation, proposal.clone(), ppml, config, linear)
        .expect("independent certificate");

    let mut forged_certificate = proposal.clone();
    forged_certificate
        .certificate
        .as_mut()
        .expect("detector claim")
        .negative_margin_relative += f64::EPSILON;
    assert!(
        validate_numerical_proposal(generation, forged_certificate, ppml, config, linear,).is_err()
    );

    let mut stale_token = validated.clone();
    stale_token.proposal.generation += 1;
    let mut wrong_reason_token = validated;
    wrong_reason_token.reason = SeparationDropReason::IteratedRectifier;
    let candidate_rows = coordinator.candidate_original_rows();
    let mut reports = Vec::new();
    reports.try_reserve_exact(8).expect("report capacity");
    let mut hashes = vec![candidate_ledger_hash_original_axis(
        &candidate_rows,
        coordinator.active(),
        coordinator.reasons(),
    )
    .expect("initial hash")];
    for wrong_token in [stale_token, wrong_reason_token] {
        let error = commit_numerical(
            &mut coordinator,
            &mut reports,
            &mut hashes,
            8,
            candidate_rows.len(),
            &candidate_rows,
            wrong_token,
            Instant::now(),
            config,
        )
        .expect_err("mismatched validation capability must fail");
        assert_eq!(error.code, ErrorCode::Internal);
    }
    assert_eq!(coordinator.view().generation, 0);
    assert_eq!(coordinator.view().original_rows, &[0, 1, 2, 3, 4, 5]);
    assert!(coordinator.reasons().iter().all(Option::is_none));
    assert!(reports.is_empty());
    assert_eq!(hashes.len(), 1);
}

#[test]
fn exact_fe_proposal_deduplicates_rows_across_absorbed_effects() {
    let (mut request, clusters, mut loaded, mut uses) = fixture();
    let h = SourceId::new(3).expect("h");
    request.absorbed_terms.push(AbsorbedTerm {
        id: AbsorbTermId::new(2).expect("second absorb"),
        intercept: true,
        categorical_sources: vec![h],
        slope_sources: Vec::new(),
        label: "h".to_owned(),
        saved_name: String::new(),
    });
    uses[2].fixed_effect = true;
    loaded.outcome = column(&[0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    let numeric = prepare_numeric(&loaded, &uses);
    let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
        .expect("two-way prepared owner");
    let coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
    let generation = coordinator.view().prepared;
    let proposal = exact_fe_proposal(generation, &[true; 6]).expect("exact FE proposal");

    // The raw zero-only memberships contain seven entries because rows
    // 0 and 2 belong to certified levels in both effects.  The proposal
    // remains a unique, bounded original-row list.
    assert_eq!(proposal.rows, vec![0, 1, 2, 3, 4]);
    assert_eq!(proposal.certificate.zero_only_levels.len(), 3);
    assert_eq!(
        proposal.certificate.zero_only_levels.capacity(),
        proposal.certificate.zero_only_levels.len(),
        "persisted exact-FE evidence must not retain detector-wide capacity",
    );
    assert!(proposal.certificate.passed);
}

#[test]
fn exact_fe_allocation_free_replay_rejects_a_deleted_level_identity() {
    let (mut request, clusters, mut loaded, mut uses) = fixture();
    let h = SourceId::new(3).expect("h");
    request.absorbed_terms.push(AbsorbedTerm {
        id: AbsorbTermId::new(2).expect("second absorb"),
        intercept: true,
        categorical_sources: vec![h],
        slope_sources: Vec::new(),
        label: "h".to_owned(),
        saved_name: String::new(),
    });
    uses[2].fixed_effect = true;
    loaded.outcome = column(&[0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    let numeric = prepare_numeric(&loaded, &uses);
    let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
        .expect("two-way prepared owner");
    let coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
    let generation = coordinator.view().prepared;
    let active = [true; 6];
    let proposal = exact_fe_proposal(generation, &active).expect("exact FE proposal");
    let topology = DetectorTopology::new(generation.prepared()).expect("topology");
    let mut missing_identity = proposal.certificate.zero_only_levels.clone();
    missing_identity.remove(1);
    let mut replay_selection = vec![false; generation.original_rows().len()];

    let error = verify_exact_fe_proposal_with_activity(
        generation,
        &topology,
        Some(&active),
        &proposal.rows,
        &missing_identity,
        proposal.certificate.checked_intercept_effects,
        &mut replay_selection,
    )
    .expect_err("a strict subset of exact level identities must fail replay");
    assert_eq!(error.code, ErrorCode::NonConvergence);
}

#[test]
fn repeated_interaction_exact_ids_survive_rebase_code_reuse_and_shared_anchors() {
    let (mut request, _clusters, mut loaded, mut uses) = fixture();
    let g = SourceId::new(2).expect("g");
    let h = SourceId::new(3).expect("h");
    request.absorbed_terms = vec![
        AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("interaction one"),
            intercept: true,
            categorical_sources: vec![g, h],
            slope_sources: Vec::new(),
            label: "g#h".to_owned(),
            saved_name: String::new(),
        },
        AbsorbedTerm {
            id: AbsorbTermId::new(2).expect("interaction two"),
            intercept: true,
            categorical_sources: vec![h, g],
            slope_sources: Vec::new(),
            label: "h#g".to_owned(),
            saved_name: String::new(),
        },
        AbsorbedTerm {
            id: AbsorbTermId::new(3).expect("interaction three"),
            intercept: true,
            categorical_sources: vec![g, h, g, h],
            slope_sources: Vec::new(),
            label: "g#h#g#h".to_owned(),
            saved_name: String::new(),
        },
    ];
    uses[1].fixed_effect = true;
    uses[2] = NumericSourceUse::fixed_effect();
    loaded.outcome = column(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0]);
    loaded.numeric_sources[1].1 = column(&[1.0, 1.0, 1.0, 1.0, 2.0, 2.0]);
    loaded.numeric_sources[2].1 = column(&[10.0, 10.0, 20.0, 20.0, 10.0, 10.0]);
    let numeric = prepare_numeric(&loaded, &uses);
    let owner = PreparedPpmlModelOwner::new(request, Vec::new(), loaded, numeric)
        .expect("repeated-interaction prepared owner");
    let mut coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");

    let first = {
        let generation = coordinator.view().prepared;
        let topology = DetectorTopology::new(generation.prepared()).expect("topology zero");
        assert_eq!(
            topology.sources.len(),
            1,
            "reordered/repeated recipes reuse one source"
        );
        assert!(topology.effect_to_source.iter().all(|source| *source == 0));
        let code_zero_anchor = generation.original_rows()[topology.sources[0]
            .checked_members(0)
            .expect("initial code zero")[0]];
        let proposal = exact_fe_proposal_all(generation).expect("initial exact proposal");
        assert_eq!(code_zero_anchor, 0);
        assert_eq!(proposal.rows, vec![0, 1]);
        assert_eq!(proposal.certificate.zero_only_levels.len(), 3);
        assert!(proposal
            .certificate
            .zero_only_levels
            .iter()
            .all(|identity| identity.representative_original_row == 0));
        proposal
    };
    coordinator
        .transact_validated_proposal(
            SeparationDropReason::FixedEffectZeroLevel,
            first.rows,
            PreparedSeparationCascade::default(),
        )
        .expect("first exact rebase");

    {
        let generation = coordinator.view().prepared;
        let topology = DetectorTopology::new(generation.prepared()).expect("topology one");
        assert_eq!(generation.original_rows(), &[2, 3, 4, 5]);
        assert_eq!(topology.sources.len(), 1);
        assert_eq!(
            generation.original_rows()[topology.sources[0]
                .checked_members(0)
                .expect("rebased code zero")[0]],
            2,
            "compact interaction code zero is legitimately reused by a new stable level",
        );
    }
    // Remove the sole positive member of the newly code-zero interaction
    // level through a separately certified primary transaction. Its
    // remaining zero member must then be identified by stable term/row
    // evidence, not confused with the generation-zero code-zero level.
    coordinator
        .transact_validated_proposal(
            SeparationDropReason::Simplex,
            vec![2],
            PreparedSeparationCascade::default(),
        )
        .expect("positive-row rebase");
    let second = {
        let generation = coordinator.view().prepared;
        let topology = DetectorTopology::new(generation.prepared()).expect("topology two");
        assert_eq!(generation.original_rows(), &[3, 4, 5]);
        assert_eq!(
            generation.original_rows()[topology.sources[0]
                .checked_members(0)
                .expect("second rebased code zero")[0]],
            3,
        );
        exact_fe_proposal_all(generation).expect("rebased exact proposal")
    };
    assert_eq!(second.rows, vec![0]);
    assert_eq!(second.certificate.zero_only_levels.len(), 3);
    assert!(second
        .certificate
        .zero_only_levels
        .iter()
        .all(|identity| identity.representative_original_row == 3));
    assert!(first
        .certificate
        .zero_only_levels
        .iter()
        .zip(&second.certificate.zero_only_levels)
        .all(|(old, new)| old.effect == new.effect
            && old.representative_original_row != new.representative_original_row));
}

#[test]
fn exact_fe_level_compaction_failure_precedes_any_ledger_mutation() {
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
    FAIL_EXACT_FE_LEVEL_COMPACTION.set(true);
    let error = match exact_fe_proposal(coordinator.view().prepared, &[true; 6]) {
        Ok(_) => panic!("injected certificate compaction must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code, ErrorCode::Allocation);
    assert_eq!(coordinator.view().generation, 0);
    assert_eq!(coordinator.view().original_rows, &[0, 1, 2, 3, 4, 5]);
    assert!(coordinator.active().iter().all(|active| *active));
    assert!(coordinator.reasons().iter().all(Option::is_none));
}

#[test]
fn singleton_pass_deduplicates_rows_across_absorbed_effects() {
    let (mut request, clusters, loaded, mut uses) = fixture();
    let h = SourceId::new(3).expect("h");
    request.absorbed_terms.push(AbsorbedTerm {
        id: AbsorbTermId::new(2).expect("second absorb"),
        intercept: true,
        categorical_sources: vec![h],
        slope_sources: Vec::new(),
        label: "h".to_owned(),
        saved_name: String::new(),
    });
    uses[2].fixed_effect = true;
    let numeric = prepare_numeric(&loaded, &uses);
    let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
        .expect("two-way prepared owner");
    let coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");
    let generation = coordinator.view().prepared;
    let topology = DetectorTopology::new(generation.prepared()).expect("topology");

    // Row zero is the sole active member of a level in both absorbed
    // effects. The pass emits it once and never grows beyond N.
    let rows = singleton_fixed_point(
        &topology,
        generation,
        &[true, false, false, false, false, false],
    )
    .expect("singleton pass");
    assert_eq!(rows, vec![0]);
}

#[test]
fn simplex_report_bank_grows_past_the_preflighted_generations() {
    let zeros = PREFLIGHT_REPORT_GENERATIONS + 1;
    let rows = zeros + 8;
    let (mut request, _, mut loaded, _) = fixture();
    let template = request.model.terms[0].clone();
    request.model.sources.clear();
    request.model.terms.clear();
    request.absorbed_terms.clear();
    request.bindings.truncate(2);
    request.keep_singletons = true;
    loaded.candidate = vec![true; rows].into();
    loaded.outcome = column(&(0..rows).map(|i| f64::from(i >= zeros)).collect::<Vec<_>>());
    loaded.numeric_sources.clear();
    for j in 0..zeros {
        let id = u32::try_from(j + 1).unwrap();
        let source_id = SourceId::new(id).unwrap();
        let name = format!("x{id}");
        request.model.sources.push(source(source_id, &name));
        let mut term = template.clone();
        term.id = TermId::new(id).unwrap();
        term.order = TermOrder::new(id).unwrap();
        term.coefficient_slot = CoefficientSlot::new(id).unwrap();
        term.canonical_token = format!("c.{name}");
        term.components[0].source = source_id;
        request.model.terms.push(term);
        request.bindings.push(OlsInputBinding {
            stata_var_position: id + 2,
            role: OlsInputRole::NumericSource(source_id),
        });
        loaded.numeric_sources.push((
            source_id,
            column(&(0..rows).map(|i| f64::from(i == j)).collect::<Vec<_>>()),
        ));
    }
    let numeric = prepare_numeric(&loaded, &vec![NumericSourceUse::ordinary(); zeros]);
    let owner = PreparedPpmlModelOwner::new(request, vec![], loaded, numeric).unwrap();
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: false,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let result = detect_prepared_ppml_separation(owner, PpmlConfig::default(), config, None, None)
        .expect("one signed indicator certificate per generation, then a terminal report");
    assert_eq!(result.separation.generations, zeros);
    assert_eq!(result.separation.methods.len(), zeros + 1);
    assert_eq!(
        result.prepared.original_rows(),
        &(zeros..rows).collect::<Vec<_>>()
    );
    assert_eq!(
        result.separation.dropped_observations,
        (0..zeros).collect::<Vec<_>>()
    );
    for report in &result.separation.methods[..zeros] {
        assert_eq!(report.dropped_observations.len(), 1);
        assert!(report.dropped_observations[0] < zeros);
        assert_eq!(report.status, MethodStatus::Certified);
    }
    result
        .certification_seal
        .validate(
            config,
            &result.separation,
            result.prepared.original_rows(),
            result.prepared.outcome(),
        )
        .unwrap();
}

#[test]
fn exact_then_comprehensive_ir_rebuilds_fe_and_cluster_topology_twice() {
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let result = detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: true,
                simplex: true,
                iterated_rectifier: true,
                mu: false,
            },
            drop_singletons: true,
            ..SeparationConfig::default()
        },
        None,
        None,
    )
    .expect("prepared separation");

    assert_eq!(result.prepared.generation(), 2);
    assert_eq!(result.prepared.original_rows(), &[4, 5]);
    assert_eq!(result.prepared.outcome(), &[1.0, 0.0]);
    assert_eq!(result.prepared.cluster_codes(0).expect("cluster"), &[0, 1]);
    assert_eq!(
        result
            .prepared
            .cluster_design()
            .expect("cluster design")
            .dimensions()[0]
            .groups(),
        2
    );
    assert_eq!(result.separation.generations, 2);
    assert_eq!(result.separation.generation_hashes.len(), 3);
    let admitted_report_records = result
        .separation
        .candidate_observations
        .checked_mul(4)
        .expect("small report envelope");
    assert_eq!(
        result.separation.methods.capacity(),
        admitted_report_records,
        "the pre-reserved method bank must not grow during multigeneration follow-ups",
    );
    assert_eq!(
        result.separation.generation_hashes.capacity(),
        result.separation.candidate_observations,
        "the generation-hash bank must retain its admitted capacity",
    );
    let mut persisted_exact_levels = 0_usize;
    let mut stable_exact_identities = std::collections::BTreeSet::new();
    for report in &result.separation.methods {
        assert_eq!(
            report.dropped_observations.capacity(),
            report.dropped_observations.len(),
            "accepted method rows must not retain detector-wide capacity",
        );
        assert_eq!(
            report.singleton_drops.capacity(),
            report.singleton_drops.len(),
            "accepted singleton rows must not retain detector-wide capacity",
        );
        if let Some(certificate) = &report.exact_fe_certificate {
            assert_eq!(
                certificate.zero_only_levels.capacity(),
                certificate.zero_only_levels.len(),
                "multigeneration exact-FE reports must retain exact capacity",
            );
            if report.status == MethodStatus::Certified {
                persisted_exact_levels += certificate.zero_only_levels.len();
                for identity in &certificate.zero_only_levels {
                    assert!(
                        stable_exact_identities.insert(*identity),
                        "a stable term/original-row identity cannot recur after rebasing",
                    );
                }
            }
        }
    }
    assert!(
        persisted_exact_levels <= 3,
        "accepted exact-FE level identities are disjoint across generations",
    );
    assert_eq!(
        result.separation.drop_reasons[0],
        Some(SeparationDropReason::FixedEffectZeroLevel)
    );
    assert_eq!(
        result.separation.drop_reasons[1],
        Some(SeparationDropReason::FixedEffectZeroLevel)
    );
    assert_eq!(
        result.separation.drop_reasons[2],
        Some(SeparationDropReason::SingletonCascade)
    );
    assert_eq!(
        result.separation.drop_reasons[3],
        Some(SeparationDropReason::Simplex)
    );
    assert!(result.separation.methods.iter().any(|report| {
        report.method == SeparationMethod::FixedEffects
            && report.status == MethodStatus::Certified
            && report.generation == 1
    }));
    assert!(result.separation.methods.iter().any(|report| {
        report.method == SeparationMethod::Simplex
            && report.status == MethodStatus::Certified
            && report.generation == 2
            && report.singleton_drops == vec![2]
    }));
    assert!(result.separation.methods.iter().any(|report| {
        report.method == SeparationMethod::Simplex
            && report.status == MethodStatus::Certified
            && report
                .certificate
                .as_ref()
                .is_some_and(|certificate| certificate.passed)
    }));
}

#[test]
fn over_budget_prepared_simplex_requires_ir_or_returns_allocation() {
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let error = detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: true,
                simplex: true,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: true,
            simplex_max_tableau_cells: 1,
            ..SeparationConfig::default()
        },
        None,
        None,
    )
    .err()
    .expect("simplex-only memory skip must fail");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn prepared_simplex_executes_the_full_absorbed_slope_model() {
    let (mut request, clusters, loaded, mut uses) = fixture();
    request.absorbed_terms[0]
        .slope_sources
        .push(SourceId::new(1).expect("x"));
    uses[0].fixed_effect = true;
    let numeric = prepare_numeric(&loaded, &uses);
    let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
        .expect("prepared slope owner");
    let result = detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: true,
                iterated_rectifier: true,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        },
        None,
        None,
    )
    .expect("bounded full-model slope simplex");
    let simplex = result
        .separation
        .methods
        .iter()
        .find(|report| report.method == SeparationMethod::Simplex)
        .expect("simplex report");
    assert_ne!(simplex.status, MethodStatus::SkippedUnsupported);
    assert_ne!(simplex.status, MethodStatus::SkippedMemory);
    assert!(simplex.simplex_tableau_cells > 0);
}

#[test]
fn prepared_simplex_alone_closes_nontrivial_no_separation_with_dual_proof() {
    let (mut request, mut clusters, mut loaded, mut uses) = fixture();
    request.absorbed_terms.clear();
    request.keep_singletons = true;
    let g = SourceId::new(2).expect("g");
    clusters.push(OlsClusterTermV1 {
        label: "g".to_owned(),
        sources: vec![g],
    });
    uses[1] = NumericSourceUse {
        ordinary: false,
        fixed_effect: false,
        cluster: true,
    };
    loaded.outcome = column(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    loaded.numeric_sources[0].1 = column(&[0.0, 1.0, -1.0, 1.0, -1.0, 1.0]);
    let numeric = prepare_numeric(&loaded, &uses);
    let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
        .expect("finite-only prepared owner");
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: false,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let result = detect_prepared_ppml_separation(owner, PpmlConfig::default(), config, None, None)
        .expect("simplex-only Farkas closure");
    assert_eq!(result.prepared.generation(), 0);
    assert!(result.separation.dropped_observations.is_empty());
    let simplex = result
        .separation
        .methods
        .iter()
        .find(|report| report.method == SeparationMethod::Simplex)
        .expect("simplex report");
    assert_eq!(simplex.status, MethodStatus::NoSeparation);
    assert!(matches!(
        simplex
            .certificate
            .as_ref()
            .map(|certificate| certificate.proof),
        Some(
            crate::ppml_separation::SeparationCertificateProof::SimplexDualInfeasibility {
                boundary_rows: 5,
                nullity: 1,
            }
        )
    ));
    result
        .certification_seal
        .validate(
            config,
            &result.separation,
            result.prepared.original_rows(),
            result.prepared.outcome(),
        )
        .expect("dual proof replay seal");
}

#[test]
fn no_separation_is_an_identity_generation_on_the_candidate_axis() {
    let (request, clusters, mut loaded, uses) = fixture();
    loaded.outcome = column(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let result = detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig::default(),
        None,
        None,
    )
    .expect("prepared no-separation result");

    assert_eq!(result.prepared.generation(), 0);
    assert_eq!(result.prepared.original_rows(), &[0, 1, 2, 3, 4, 5]);
    assert_eq!(result.candidate_original_rows.as_ref(), &[0, 1, 2, 3, 4, 5]);
    assert_eq!(result.separation.active, vec![true; 6]);
    assert_eq!(
        result.separation.retained_observations,
        vec![0, 1, 2, 3, 4, 5]
    );
    assert!(result.separation.dropped_observations.is_empty());
    assert!(result.separation.drop_reasons.iter().all(Option::is_none));
    assert_eq!(result.separation.generation_hashes.len(), 1);
    assert_eq!(
        result
            .separation
            .methods
            .iter()
            .map(|report| (report.method, report.status))
            .collect::<Vec<_>>(),
        vec![
            (SeparationMethod::Singletons, MethodStatus::NoSeparation),
            (SeparationMethod::FixedEffects, MethodStatus::NoSeparation),
            (SeparationMethod::Simplex, MethodStatus::NoCandidates),
            (
                SeparationMethod::IteratedRectifier,
                MethodStatus::NoCandidates,
            ),
        ]
    );
}

#[test]
fn disabled_detector_chain_preserves_boundary_rows() {
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let result = detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        },
        None,
        None,
    )
    .expect("disabled separation chain");

    assert_eq!(result.prepared.generation(), 0);
    assert_eq!(result.prepared.original_rows(), &[0, 1, 2, 3, 4, 5]);
    assert!(result.separation.methods.is_empty());
    assert_eq!(result.separation.active, vec![true; 6]);
}

#[test]
fn prepared_ir_is_comprehensive_without_the_simplex_adapter() {
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: true,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let result = detect_prepared_ppml_separation(owner, PpmlConfig::default(), config, None, None)
        .expect("prepared IR separation");

    assert_eq!(result.separation.dropped_observations, vec![0, 1, 3]);
    assert_eq!(result.prepared.original_rows(), &[2, 4, 5]);
    assert!(result.separation.methods.iter().any(|report| {
        report.method == SeparationMethod::IteratedRectifier
            && report.status == MethodStatus::Certified
            && report.dropped_observations == vec![0, 1, 3]
            && report
                .certificate
                .as_ref()
                .is_some_and(|value| value.passed)
    }));
    assert!(result.separation.methods.iter().any(|report| {
        report.method == SeparationMethod::IteratedRectifier
            && report.status == MethodStatus::NoSeparation
            && report.certificate.as_ref().is_some_and(|certificate| {
                certificate.positive_maximum >= 0.0
                    && certificate.boundary_violation_relative >= 0.0
            })
    }));
    result
        .certification_seal
        .validate(
            config,
            &result.separation,
            result.prepared.original_rows(),
            result.prepared.outcome(),
        )
        .expect("terminal IR replay seal");

    let mut mutated = result.separation.clone();
    let terminal_ir = mutated
        .methods
        .iter_mut()
        .rev()
        .find(|report| {
            report.method == SeparationMethod::IteratedRectifier
                && report.status == MethodStatus::NoSeparation
        })
        .expect("terminal IR fixed-point report");
    terminal_ir
        .certificate
        .as_mut()
        .expect("terminal IR certificate")
        .positive_maximum += f64::EPSILON;
    assert_eq!(
        result
            .certification_seal
            .validate(
                config,
                &mutated,
                result.prepared.original_rows(),
                result.prepared.outcome(),
            )
            .expect_err("mutated pass-through IR certificate must not match the replay seal")
            .code,
        ErrorCode::Internal
    );

    let disabled = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: false,
        },
        ..config
    };
    assert!(result
        .certification_seal
        .validate(
            disabled,
            &result.separation,
            result.prepared.original_rows(),
            result.prepared.outcome(),
        )
        .is_err());
}

#[test]
fn prepared_mu_uses_original_axis_start_and_certifies_its_candidate() {
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let start = PpmlStart {
        coefficients: Vec::new(),
        linear_predictor: vec![0.0, 0.0, 0.0, -30.0, 0.0, 0.0],
    };
    let result = detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: false,
                mu: true,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        },
        None,
        Some(&start),
    )
    .expect("prepared mu separation");

    assert_eq!(result.separation.dropped_observations, vec![3]);
    assert_eq!(result.prepared.original_rows(), &[0, 1, 2, 4, 5]);
    assert_eq!(
        result.separation.drop_reasons[3],
        Some(SeparationDropReason::Mu)
    );
    assert!(result.separation.methods.iter().any(|report| {
        report.method == SeparationMethod::Mu
            && report.status == MethodStatus::Certified
            && report.dropped_observations == vec![3]
            && report
                .certificate
                .as_ref()
                .is_some_and(|value| value.passed)
    }));
    let terminal_mu = result
        .separation
        .methods
        .last()
        .expect("terminal Mu replay");
    assert_eq!(terminal_mu.method, SeparationMethod::Mu);
    assert_eq!(terminal_mu.status, MethodStatus::NoCandidates);
    assert_eq!(terminal_mu.iterations, 0);
    assert!(terminal_mu.dropped_observations.is_empty());
    assert!(terminal_mu.certificate.is_none());
}

#[test]
fn default_chain_is_deterministic_and_uses_the_declared_restart_order() {
    let run = || {
        let (request, clusters, loaded, uses) = fixture();
        let numeric = prepare_numeric(&loaded, &uses);
        let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
            .expect("prepared owner");
        detect_prepared_ppml_separation(
            owner,
            PpmlConfig::default(),
            SeparationConfig::default(),
            None,
            None,
        )
        .expect("default prepared separation")
    };
    let first = run();
    let second = run();
    let signature = |outcome: &PreparedPpmlSeparationOutcome| {
        outcome
            .separation
            .methods
            .iter()
            .map(|report| {
                (
                    report.generation,
                    report.method,
                    report.status,
                    report.dropped_observations.clone(),
                    report.singleton_drops.clone(),
                )
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(first.prepared.original_rows(), &[4, 5]);
    assert_eq!(
        first.separation.generation_hashes,
        second.separation.generation_hashes
    );
    assert_eq!(
        first.separation.drop_reasons,
        second.separation.drop_reasons
    );
    assert_eq!(signature(&first), signature(&second));
    assert_eq!(
        signature(&first)
            .iter()
            .map(|(_, method, _, _, _)| *method)
            .collect::<Vec<_>>(),
        vec![
            SeparationMethod::Singletons,
            SeparationMethod::FixedEffects,
            SeparationMethod::Singletons,
            SeparationMethod::FixedEffects,
            SeparationMethod::Simplex,
            SeparationMethod::Singletons,
            SeparationMethod::FixedEffects,
            SeparationMethod::Simplex,
            SeparationMethod::IteratedRectifier,
        ]
    );
}

#[test]
fn failed_prepared_rebase_leaves_generation_and_ledger_unchanged() {
    let (request, clusters, loaded, uses) = fixture();
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let mut coordinator = PreparedSeparationCoordinator::new(owner).expect("coordinator");

    let error = coordinator
        .transact_validated_proposal(
            SeparationDropReason::Simplex,
            vec![1, 3, 5],
            PreparedSeparationCascade::default(),
        )
        .expect_err("one cluster group must reject the staged generation");
    assert_eq!(error.code, ErrorCode::InvalidData);
    assert_eq!(coordinator.view().generation, 0);
    assert_eq!(coordinator.view().original_rows, &[0, 1, 2, 3, 4, 5]);
    assert!(coordinator.reasons().iter().all(Option::is_none));
    assert!(coordinator.active().iter().all(|active| *active));

    coordinator
        .transact_validated_proposal(
            SeparationDropReason::Simplex,
            vec![0],
            PreparedSeparationCascade::default(),
        )
        .expect("valid transaction after rollback");
    assert_eq!(coordinator.view().generation, 1);
    assert_eq!(coordinator.view().original_rows, &[1, 2, 3, 4, 5]);
}

#[test]
fn accepted_cascade_rows_do_not_retain_detector_sized_capacity() {
    let mut detector_rows = Vec::new();
    detector_rows
        .try_reserve_exact(128)
        .expect("detector row workspace");
    detector_rows.extend([1_usize, 4, 7]);
    assert!(detector_rows.capacity() >= 128);
    let original_rows = (10_usize..18).collect::<Vec<_>>();
    let accepted = compact_to_original_exact_capacity(detector_rows, &original_rows)
        .expect("accepted exact-capacity cascade rows");
    assert_eq!(accepted, [11, 14, 17]);
    assert_eq!(
        accepted.capacity(),
        accepted.len(),
        "persisted method reports must scale with deleted identities, not prior generation N",
    );
}

#[test]
fn public_report_is_candidate_compact_while_final_reasons_use_original_rows() {
    let (request, clusters, mut loaded, uses) = fixture();
    loaded.candidate.explicit_mut().unwrap()[0] = false;
    let numeric = prepare_numeric(&loaded, &uses);
    let owner =
        PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).expect("prepared owner");
    let result = detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: true,
                simplex: false,
                iterated_rectifier: false,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        },
        None,
        None,
    )
    .expect("candidate-axis separation");

    assert_eq!(result.candidate_original_rows.as_ref(), &[1, 2, 3, 4, 5]);
    assert_eq!(result.separation.candidate_observations, 5);
    assert_eq!(result.separation.dropped_observations, vec![0]);
    assert_eq!(
        result.separation.drop_reasons[0],
        Some(SeparationDropReason::FixedEffectZeroLevel)
    );
    assert_eq!(
        result.original_axis_reasons[1],
        Some(SeparationDropReason::FixedEffectZeroLevel)
    );
    assert_eq!(result.original_axis_reasons[0], None);
    let final_reasons = result.final_drop_reasons().expect("merged original ledger");
    assert_eq!(final_reasons[0], DropReason::ExcludedByIfIn);
    assert_eq!(final_reasons[1], DropReason::FixedEffectSeparation);
}

#[test]
fn separation_rebase_preserves_offset_cluster_and_final_pweight_scale() {
    let (mut request, clusters, mut loaded, uses) = fixture();
    request.bindings.push(OlsInputBinding {
        stata_var_position: 6,
        role: OlsInputRole::ProbabilityWeight,
    });
    loaded.probability_weight = Some(column(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]));
    let offset = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5];
    let probability = loaded.probability_weight.as_ref().expect("pweights");
    let numeric = prepare_numeric_with(
        &loaded,
        &uses,
        WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::with_missing(&probability.values, &probability.missing),
        },
        Some(NumericColumnView::new(&offset)),
    );
    let owner = PreparedPpmlModelOwner::new(request, clusters, loaded, numeric)
        .expect("prepared weighted owner");
    let result = detect_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: true,
                simplex: true,
                iterated_rectifier: true,
                mu: false,
            },
            drop_singletons: true,
            ..SeparationConfig::default()
        },
        None,
        None,
    )
    .expect("weighted prepared separation");

    assert_eq!(result.prepared.original_rows(), &[4, 5]);
    assert_eq!(result.prepared.offset(), &[0.4, 0.5]);
    assert_eq!(result.prepared.cluster_codes(0).expect("codes"), &[0, 1]);
    assert_eq!(
        result
            .prepared
            .cluster_design()
            .expect("cluster design")
            .dimensions()[0]
            .groups(),
        2
    );
    let weights = result.prepared.prior_weights();
    assert!((weights.iter().sum::<f64>() - 2.0).abs() < 1e-12);
    assert_eq!(result.prepared.weight_report().physical_observations, 2);
    assert!((result.prepared.weight_report().prepared_weight_sum - 2.0).abs() < 1e-12);
    assert!(
        (result
            .prepared
            .weight_report()
            .raw_weight_sum
            .expect("raw sum")
            - 11.0)
            .abs()
            < 1e-12
    );
    assert!(
        (result
            .prepared
            .weight_report()
            .raw_to_prepared_multiplier
            .expect("representable multiplier")
            - 2.0 / 11.0)
            .abs()
            < 1e-12
    );

    let weight_sum = weights.iter().sum::<f64>();
    let divisor = weights.iter().copied().fold(0.0_f64, f64::max);
    let frame = LinearSolveFrame::from_parts(
        result.prepared.outcome(),
        BorrowedMapWeights::Provided(weights),
        PreparedOlsBreadWeightScale {
            weight_kind: Some(WeightKind::Probability),
            solver_weight_divisor: divisor,
            reference_weight_multiplier: result
                .prepared
                .weight_report()
                .raw_to_prepared_multiplier
                .expect("representable projection multiplier"),
            reference_weight_sum: weight_sum,
        },
    )
    .expect("rank frame");
    let fit = fit_prepared_weighted_linear_projection_once(
        result.prepared.fit_view(),
        frame,
        PreparedOlsFitConfig::default(),
    )
    .expect("rank-certified final generation");
    assert_eq!(fit.finite_rank(), 0);
}

#[test]
fn rectifier_fixed_point_evidence_replaces_the_rerun_and_rejects_corruption() {
    // Zeros at rows 0 and 3 with no separating direction: level 3 has two
    // positive rows, which forces the regressor coefficient to zero.
    let (request, clusters, mut loaded, uses) = fixture();
    loaded.outcome = column(&[0., 2., 1., 0., 1., 3.]);
    loaded.numeric_sources[0].1 = column(&[0.5, -1., 2., -0.3, 1., 0.7]);
    let owner = PreparedPpmlModelOwner::new(
        request,
        clusters,
        loaded.clone(),
        prepare_numeric(&loaded, &uses),
    )
    .unwrap();
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: true,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let ppml = PpmlConfig::default();
    let linear = PreparedOlsFitConfig {
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    let generation = coordinator.view().prepared;
    let proposal =
        prepared_bound_iterated_rectifier(generation, ppml, config, linear, false, &mut |_| Ok(()))
            .unwrap();
    assert_eq!(proposal.status, MethodStatus::NoSeparation);
    let evidence = proposal.fixed_point.clone().expect("terminal evidence");
    assert_eq!(
        certify_ir_fixed_point_evidence(generation, &evidence, config).unwrap(),
        proposal.certificate
    );
    let replay = |observed: &PreparedNumericalProposal| {
        replay_numerical_no_drop(
            generation,
            observed,
            ppml,
            config,
            linear,
            false,
            &mut |_| Ok(()),
            None,
        )
        .unwrap()
    };
    assert_eq!(replay(&proposal), proposal.certificate);

    // A residual moved off the projection along the regressor keeps the
    // sign conditions but leaves a regressor moment; a larger move breaks
    // the sign conditions; claiming zero fitted values leaves this
    // fixture's FE moments unexplained.
    let x = [0.5, -1., 2., -0.3, 1., 0.7];
    let mut moment_shift = evidence.clone();
    let mut sign_shift = evidence.clone();
    for row in [1, 2, 4, 5] {
        moment_shift.residual[row] += 1e-9 * x[row];
        sign_shift.residual[row] += 1e-3 * x[row];
    }
    assert!(
        assess_ir_projection(
            generation.outcome(),
            |row| moment_shift.recurrence[row] - moment_shift.residual[row],
            config,
        )
        .certificate
        .is_some(),
        "the small shift must pass the sign checks"
    );
    let mut zero_fit = evidence.clone();
    zero_fit.residual.clone_from(&zero_fit.recurrence);
    let mut unnormalized = evidence.clone();
    unnormalized
        .recurrence
        .iter_mut()
        .for_each(|value| *value *= 0.5);
    let mut positive_support = evidence.clone();
    positive_support.recurrence[1] = 0.25;
    let mut nonfinite_residual = evidence.clone();
    nonfinite_residual.residual[2] = f64::NAN;
    let mut foreign_column = evidence.clone();
    foreign_column.kept_columns.push(usize::MAX);
    let mut zero_recurrence_with_residual = evidence.clone();
    zero_recurrence_with_residual.recurrence.fill(0.0);
    for corrupted in [
        moment_shift,
        sign_shift,
        zero_fit,
        unnormalized,
        positive_support,
        nonfinite_residual,
        foreign_column,
        zero_recurrence_with_residual,
    ] {
        assert!(
            certify_ir_fixed_point_evidence(generation, &corrupted, config)
                .unwrap()
                .is_none()
        );
        // Failed evidence falls back to the deterministic re-run, which
        // still reproduces the reported certificate.
        let mut fallback = proposal.clone();
        fallback.fixed_point = Some(corrupted);
        assert_eq!(replay(&fallback), proposal.certificate);
    }

    // A recurrence that every boundary row has left is the vanishing
    // projection, certified without moments.
    let vanished = IrFixedPointEvidence {
        recurrence: vec![0.0; 6],
        residual: vec![0.0; 6],
        kept_columns: evidence.kept_columns.clone(),
    };
    let vanished_certificate = certify_ir_fixed_point_evidence(generation, &vanished, config)
        .unwrap()
        .expect("vanishing projection");
    assert_eq!(vanished_certificate.direction_scale, 0.0);
    assert_eq!(vanished_certificate.interior_zero_relative, 0.0);

    // Metadata the evidence cannot vouch for skips the fast path, so the
    // re-run's equality checks still reject it.
    let mut wrong_iterations = proposal.clone();
    wrong_iterations.iterations = 0;
    assert_eq!(
        replay_numerical_no_drop(
            generation,
            &wrong_iterations,
            ppml,
            config,
            linear,
            false,
            &mut |_| Ok(()),
            None,
        )
        .unwrap_err()
        .code,
        ErrorCode::NonConvergence
    );
}

#[test]
fn rectifier_projection_certifies_the_deferred_intercept_under_a_slope_fe() {
    // feppml y x, absorb(g##c.x) with zero outcomes. The projection-only
    // fit leaves the absorbed intercept deferred. Under a slope FE its
    // within norm can be solver roundoff (about 3e-16 here) instead of
    // exactly zero, and dividing its raw score moment by that norm failed
    // the projection's score certificate with a relative moment near 2e-6.
    // Here x itself is also absorbed and omitted.
    let (mut request, clusters, mut loaded, mut uses) = fixture();
    let x = SourceId::new(1).unwrap();
    request.absorbed_terms[0].slope_sources = vec![x];
    uses[0].fixed_effect = true;
    loaded.candidate = vec![true; 12].into();
    loaded.outcome = column(&[0., 1., 2., 3., 4., 0., 1., 2., 3., 1., 0., 2.]);
    loaded.numeric_sources[0].1 =
        column(&[0.5, 1., -1., 2., 1.5, -0.5, 0.25, 3., 2., -1., 1., 0.75]);
    loaded.numeric_sources[1].1 = column(&[1., 1., 1., 1., 2., 2., 2., 2., 3., 3., 3., 3.]);
    loaded.numeric_sources[2].1 =
        column(&[10., 20., 10., 20., 10., 20., 10., 20., 10., 20., 10., 20.]);
    let make_owner = || {
        PreparedPpmlModelOwner::new(
            request.clone(),
            clusters.clone(),
            loaded.clone(),
            prepare_numeric(&loaded, &uses),
        )
        .unwrap()
    };
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: true,
            simplex: false,
            iterated_rectifier: true,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let ppml = PpmlConfig::default();
    let linear = PreparedOlsFitConfig {
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    let coordinator = PreparedSeparationCoordinator::new(make_owner()).unwrap();
    let generation = coordinator.view().prepared;
    let proposal =
        prepared_bound_iterated_rectifier(generation, ppml, config, linear, false, &mut |_| Ok(()))
            .expect("absorbed regressor must not fail the rectifier projection");
    assert_eq!(proposal.status, MethodStatus::NoSeparation);
    let evidence = proposal.fixed_point.as_ref().expect("terminal evidence");
    assert!(evidence.kept_columns.is_empty());
    assert_eq!(
        certify_ir_fixed_point_evidence(generation, evidence, config).unwrap(),
        proposal.certificate
    );
    let result = detect_prepared_ppml_separation(make_owner(), ppml, config, None, None)
        .expect("separation detection");
    assert_eq!(result.separation.separated_count, 0);
}

#[test]
fn rectifier_fixed_point_evidence_checks_fe_slope_channels() {
    // g##c.h with an intercept: each level's three positive rows have
    // distinct h, so no direction separates its zero row.
    let (mut request, clusters, mut loaded, mut uses) = fixture();
    let h = SourceId::new(3).unwrap();
    request.absorbed_terms[0].slope_sources = vec![h];
    uses[2].fixed_effect = true;
    loaded.candidate = vec![true; 12].into();
    loaded.outcome = column(&[0., 1., 2., 3., 4., 0., 1., 2., 3., 1., 0., 2.]);
    let xs = [0.5, 1., -1., 2., 1.5, -0.5, 0.25, 3., 2., -1., 1., 0.75];
    loaded.numeric_sources[0].1 = column(&xs);
    loaded.numeric_sources[1].1 = column(&[1., 1., 1., 1., 2., 2., 2., 2., 3., 3., 3., 3.]);
    let hs = [
        0.3, 1.7, -0.8, 2.4, 1.1, -0.6, 0.9, 2.2, -1.3, 0.4, 1.6, 0.7,
    ];
    loaded.numeric_sources[2].1 = column(&hs);
    let owner = PreparedPpmlModelOwner::new(
        request,
        clusters,
        loaded.clone(),
        prepare_numeric(&loaded, &uses),
    )
    .unwrap();
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: true,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let ppml = PpmlConfig::default();
    let linear = PreparedOlsFitConfig {
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    let generation = coordinator.view().prepared;
    assert_eq!(
        generation.prepared().as_ref().effect_specs()[0]
            .slopes
            .len(),
        1
    );
    let proposal =
        prepared_bound_iterated_rectifier(generation, ppml, config, linear, false, &mut |_| Ok(()))
            .unwrap();
    assert_eq!(proposal.status, MethodStatus::NoSeparation);
    let evidence = proposal.fixed_point.clone().expect("terminal evidence");
    assert_eq!(
        certify_ir_fixed_point_evidence(generation, &evidence, config).unwrap(),
        proposal.certificate
    );
    // Moving positive rows of the first level along its slope channel
    // keeps the signs but leaves that channel's moment unexplained.
    let mut slope_shift = evidence.clone();
    for row in [1, 2, 3] {
        slope_shift.residual[row] += 1e-9 * hs[row];
    }
    assert!(
        certify_ir_fixed_point_evidence(generation, &slope_shift, config)
            .unwrap()
            .is_none()
    );
}

#[test]
fn admitted_graph_direction_passes_full_span_and_terminal_ledger_checks() {
    let (mut request, clusters, mut loaded, mut uses) = fixture();
    let mut second = request.absorbed_terms[0].clone();
    second.id = AbsorbTermId::new(2).unwrap();
    second.categorical_sources = vec![SourceId::new(3).unwrap()];
    second.label = "h".into();
    request.absorbed_terms.push(second);
    request.keep_singletons = true;
    loaded.outcome = column(&[1., 1., 1., 1., 0., 0.]);
    loaded.numeric_sources[0].1 = column(&[1., -1., 1., -1., 0., 0.]);
    loaded.numeric_sources[1].1 = column(&[1., 1., 2., 2., 1., 1.]);
    loaded.numeric_sources[2].1 = column(&[1., 1., 2., 2., 2., 2.]);
    uses[2].fixed_effect = true;
    let make_owner = || {
        PreparedPpmlModelOwner::new(
            request.clone(),
            clusters.clone(),
            loaded.clone(),
            prepare_numeric(&loaded, &uses),
        )
        .unwrap()
    };
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: true,
            simplex: false,
            iterated_rectifier: true,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let mut ppml = PpmlConfig::default();
    ppml.absorb.solver = FeSolverKind::Map;
    ppml.absorb.map.max_iterations = 128;
    ppml.absorb.lsmr.threads = crate::ThreadConfig::from_requested(1, true).unwrap();
    let original = detect_prepared_ppml_separation(make_owner(), ppml, config, None, None);
    assert!(original
        .err()
        .unwrap()
        .message
        .contains("borrowed MAP exhausted"));
    ppml.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: 1_000_000,
    });
    let coordinator = PreparedSeparationCoordinator::new(make_owner()).unwrap();
    let generation = coordinator.view().prepared;
    let linear = PreparedOlsFitConfig {
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    // A generic solver budget is not authority to allocate detector state.
    let unadmitted =
        prepared_bound_iterated_rectifier(generation, ppml, config, linear, false, &mut |_| Ok(()));
    let error = unadmitted.err().unwrap();
    assert!(error.message.contains("borrowed MAP exhausted"), "{error}");
    let proposal =
        prepared_bound_iterated_rectifier(generation, ppml, config, linear, true, &mut |_| Ok(()))
            .unwrap();
    assert!(proposal.exact_intercept_span.is_some());
    let certified =
        validate_numerical_proposal(generation, proposal.clone(), ppml, config, linear).unwrap();
    assert_eq!(certified.certificate.span_residual_relative, 0.0);
    let mut malformed = proposal;
    malformed.dropped_original_rows.clear();
    malformed.direction = None;
    malformed.status = MethodStatus::NoSeparation;
    assert!(validate_prepared_numerical_metadata(generation, &malformed, config).is_err());
    let result = detect_prepared_ppml_separation(make_owner(), ppml, config, None, None).unwrap();
    assert_eq!(result.prepared.original_rows(), &[0, 1, 2, 3]);
    assert_eq!(result.separation.separated_count, 2);
    assert!(result
        .separation
        .methods
        .iter()
        .any(|r| r.method == SeparationMethod::IteratedRectifier
            && r.iterations == 0
            && r.status == MethodStatus::Certified));
}

#[test]
fn admitted_projection_still_detects_finite_separation_outside_graph_submodel() {
    for opposed in [false, true] {
        let (mut request, clusters, mut loaded, mut uses) = fixture();
        let mut second = request.absorbed_terms[0].clone();
        second.id = AbsorbTermId::new(2).unwrap();
        second.categorical_sources = vec![SourceId::new(3).unwrap()];
        second.label = "h".into();
        request.absorbed_terms.push(second);
        request.keep_singletons = true;
        loaded.outcome = column(&[1., 1., 1., 1., 0., 0.]);
        loaded.numeric_sources[0].1 = column(&[0., 0., 0., 0., 1., if opposed { -1. } else { 1. }]);
        loaded.numeric_sources[1].1 = column(&[1., 1., 2., 2., 1., 2.]);
        loaded.numeric_sources[2].1 = column(&[1., 2., 1., 2., 1., 2.]);
        uses[2].fixed_effect = true;
        let owner = PreparedPpmlModelOwner::new(
            request,
            clusters,
            loaded.clone(),
            prepare_numeric(&loaded, &uses),
        )
        .unwrap();
        let config = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: false,
                simplex: false,
                iterated_rectifier: true,
                mu: false,
            },
            drop_singletons: false,
            ..SeparationConfig::default()
        };
        let mut ppml = PpmlConfig::default();
        ppml.absorb.solver = FeSolverKind::Cg;
        ppml.absorb.retain_coefficients = true;
        ppml.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 1_000_000,
        });
        // Positive rows span the entire two-intercept submodel. Only x can
        // separate the zeros; opposite signs rule out that final direction.
        let outcome = detect_prepared_ppml_separation(owner, ppml, config, None, None).unwrap();
        assert_eq!(
            outcome.separation.separated_count,
            if opposed { 0 } else { 2 }
        );
        assert_eq!(
            outcome.prepared.original_rows(),
            if opposed {
                &[0, 1, 2, 3, 4, 5][..]
            } else {
                &[0, 1, 2, 3][..]
            }
        );
    }
}

fn fixture() -> (
    OlsRequestV1,
    Vec<OlsClusterTermV1>,
    LoadedOlsInput,
    Vec<NumericSourceUse>,
) {
    let x = SourceId::new(1).expect("x");
    let g = SourceId::new(2).expect("g");
    let h = SourceId::new(3).expect("h");
    let request = OlsRequestV1 {
        model: CompiledTermGraph {
            sources: vec![source(x, "x"), source(g, "g"), source(h, "h")],
            terms: vec![ExpandedTerm {
                id: TermId::new(1).expect("term"),
                order: TermOrder::new(1).expect("order"),
                coefficient_slot: CoefficientSlot::new(1).expect("slot"),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: "c.x".to_owned(),
                components: vec![TermComponent {
                    order: ComponentOrder::new(1).expect("component"),
                    source: x,
                    kind: ComponentKind::Continuous,
                    level_state: LevelState::Absent,
                    level_value: 0.0,
                    base: MetadataFlag::False,
                    omitted: MetadataFlag::Unavailable,
                    raw_operator: "c".to_owned(),
                    time_series_state: TimeSeriesState::None,
                    raw_time_series_operator: String::new(),
                }],
            }],
        },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("absorb"),
            intercept: true,
            categorical_sources: vec![g],
            slope_sources: Vec::new(),
            label: "g".to_owned(),
            saved_name: String::new(),
        }],
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
                role: OlsInputRole::NumericSource(x),
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::NumericSource(g),
            },
            OlsInputBinding {
                stata_var_position: 5,
                role: OlsInputRole::NumericSource(h),
            },
        ],
        include_intercept: true,
        keep_singletons: false,
    };
    let loaded = LoadedOlsInput {
        candidate: vec![true; 6].into(),
        outcome: column(&[0.0, 0.0, 1.0, 0.0, 1.0, 0.0]),
        numeric_sources: vec![
            (x, column(&[0.0, 0.0, 0.0, -1.0, 0.0, 0.0])),
            (g, column(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0])),
            (h, column(&[10.0, 20.0, 10.0, 20.0, 10.0, 20.0])),
        ],
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    (
        request,
        vec![OlsClusterTermV1 {
            label: "h".to_owned(),
            sources: vec![h],
        }],
        loaded,
        vec![
            NumericSourceUse::ordinary(),
            NumericSourceUse::fixed_effect(),
            NumericSourceUse {
                ordinary: false,
                fixed_effect: false,
                cluster: true,
            },
        ],
    )
}

fn prepare_numeric(loaded: &LoadedOlsInput, uses: &[NumericSourceUse]) -> PreparedPpmlNumericData {
    prepare_numeric_with(loaded, uses, WeightInput::None, None)
}

fn prepare_numeric_with<'a>(
    loaded: &'a LoadedOlsInput,
    uses: &[NumericSourceUse],
    weights: WeightInput<'a>,
    offset: Option<NumericColumnView<'a>>,
) -> PreparedPpmlNumericData {
    let sources = loaded
        .numeric_sources
        .iter()
        .map(|(_, source)| NumericColumnView::with_missing(&source.values, &source.missing))
        .collect::<Vec<_>>();
    prepare_ppml_numeric_data(&PpmlNumericPreparationInput {
        candidate: loaded.candidate.view(),
        outcome: NumericColumnView::with_missing(&loaded.outcome.values, &loaded.outcome.missing),
        numeric_sources: &sources,
        source_uses: uses,
        weights,
        offset,
        exposure: None,
    })
    .expect("numeric preparation")
}

fn source(id: SourceId, label: &str) -> SourceEntry {
    SourceEntry {
        id,
        canonical_name: label.to_owned(),
    }
}

fn column(values: &[f64]) -> LoadedNumericColumn {
    LoadedNumericColumn {
        values: values.to_vec(),
        missing: vec![false; values.len()],
    }
}
