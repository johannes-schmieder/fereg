use super::*;

#[test]
fn publication_frame_checks_overflow_and_simultaneous_parents() {
    let model = PpmlPublicationMemory::estimate(100, 60, 50, 10, 3).unwrap();
    assert_eq!(model.parent_bytes, 150);
    assert_eq!(
        model.shadow_bytes,
        80 + 3 * std::mem::size_of::<&[f64]>() as u128
    );
    let peak = model.peak_bytes().unwrap();
    model.admit(Some(peak)).unwrap();
    assert_eq!(
        model.admit(Some(peak - 1)).unwrap_err().code,
        ErrorCode::Allocation
    );
    assert_eq!(
        PpmlPublicationMemory::estimate(10, 200, 50, 0, 0)
            .unwrap()
            .parent_bytes,
        250
    );
    assert!(PpmlPublicationMemory::estimate(u128::MAX, 0, 1, 0, 0).is_err());
    assert!(PpmlPublicationMemory::estimate(u128::MAX, 0, 0, 1, 0).is_err());
}

#[test]
fn allocation_free_optional_cell_validation_matches_index_oracle() {
    for (rows, columns, first, last) in [
        (7, SEPARATION_COLUMNS, 8, 12),
        (SOLVER_ROWS, SOLVER_COLUMNS, 16, 22),
    ] {
        let optional: Vec<usize> = (0..rows)
            .flat_map(|row| (first..last).map(move |column| row * columns + column))
            .collect();
        let mut values = vec![0.0; rows * columns];
        for index in 0..values.len() {
            values[index] = STATA_SYSTEM_MISSING;
            let expected =
                validate_stata_cells("oracle", &values, &optional, StataCellClass::Structural)
                    .is_ok();
            let actual = validate_stata_cells_where(
                "candidate",
                &values,
                |i| (first..last).contains(&(i % columns)),
                StataCellClass::Structural,
            )
            .is_ok();
            assert_eq!(actual, expected, "cell {index} of {columns}-column wire");
            values[index] = 0.0;
        }
    }
}

#[test]
fn solver_fallback_reason_is_row_local() {
    assert_eq!(solver_row_fallback_reason(0, 1), 0.0);
    assert_eq!(solver_row_fallback_reason(1, 1), 1.0);
}

fn ledger_digest_fixture() -> SeparationReport {
    SeparationReport {
        candidate_observations: 4,
        active: vec![true, false, true, false],
        retained_observations: vec![0, 2],
        dropped_observations: vec![1, 3],
        drop_reasons: vec![
            None,
            Some(fereg_core::ppml_separation::SeparationDropReason::InitialSingleton),
            None,
            Some(fereg_core::ppml_separation::SeparationDropReason::Simplex),
        ],
        singleton_count: 1,
        separated_count: 1,
        generations: 2,
        generation_hashes: vec![0x1111_2222_3333_4444, 7, 0xaaaa_bbbb_cccc_dddd],
        methods: vec![
            fereg_core::ppml_separation::SeparationMethodReport {
                generation: 1,
                method: SeparationMethod::Singletons,
                status: MethodStatus::Certified,
                iterations: 1,
                dropped_observations: Vec::new(),
                singleton_drops: vec![1],
                certificate: None,
                exact_fe_certificate: None,
                elapsed_seconds: 0.25,
                workspace_observation_vectors: 0,
                simplex_tableau_cells: 0,
                note: "singleton fixture".into(),
            },
            fereg_core::ppml_separation::SeparationMethodReport {
                generation: 2,
                method: SeparationMethod::Simplex,
                status: MethodStatus::Certified,
                iterations: 3,
                dropped_observations: vec![3],
                singleton_drops: Vec::new(),
                certificate: Some(fereg_core::ppml_separation::SeparationCertificate {
                    proof: SeparationCertificateProof::RecessionDirection,
                    interior_zero_relative: 1.0e-12,
                    boundary_violation_relative: 2.0e-12,
                    span_residual_relative: 3.0e-12,
                    negative_margin_relative: 0.5,
                    direction_scale: 2.0,
                    positive_maximum: 0.0,
                    passed: true,
                }),
                exact_fe_certificate: None,
                elapsed_seconds: 0.5,
                workspace_observation_vectors: 0,
                simplex_tableau_cells: 128,
                note: "simplex fixture".into(),
            },
        ],
    }
}

fn valid_runtime_skeleton() -> Vec<f64> {
    let mut runtime = vec![0.0; RUNTIME_ROWS];
    runtime[16..=22].fill(STATA_SYSTEM_MISSING);
    runtime[27..40].fill(STATA_SYSTEM_MISSING);
    runtime[41..76].fill(STATA_SYSTEM_MISSING);
    runtime[40] = 0.0;
    runtime[76] = 1.0;
    runtime[77] = 1.0;
    runtime[78] = STATA_SYSTEM_MISSING;
    runtime[79..82].fill(STATA_SYSTEM_MISSING);
    runtime[95..108].fill(STATA_SYSTEM_MISSING);
    runtime
}

#[test]
fn first_action_allocator_observation_keeps_prior_baseline_missing() {
    let mut runtime = valid_runtime_skeleton();
    finalize_ppml_runtime_shadow(
        &mut runtime,
        PpmlRuntimeFinalObservation {
            total_seconds: 0.5,
            publication_extension_seconds: 0.1,
            publication_peak_bytes: None,
            action_start: Some(PpmlAllocatorSnapshot {
                current_bytes: 100,
                peak_bytes: 100,
                allocation_events: 7,
            }),
            action_end: Some(PpmlAllocatorSnapshot {
                current_bytes: 120,
                peak_bytes: 180,
                allocation_events: 10,
            }),
            previous_baseline_bytes: None,
            modeled_selected_bytes: 160,
        },
    )
    .expect("first action runtime");
    assert_eq!(runtime[16], STATA_SYSTEM_MISSING);
    assert_eq!(runtime[78], STATA_SYSTEM_MISSING);
    assert_eq!(&runtime[79..82], &[7.0, 10.0, 3.0]);
    assert_eq!(runtime[20], 80.0);
    assert_eq!(runtime[21], 20.0);
    assert_eq!(runtime[22], 0.5);
}

#[test]
fn finite_weighted_outcome_that_stata_cannot_represent_is_r430() {
    let large = 0.6 * STATA_SYSTEM_MISSING;
    let product_error = weighted_outcome_sum(&[large], &[2.0])
        .expect_err("finite product beyond Stata numeric range must fail");
    assert_eq!(product_error.code, ErrorCode::NonConvergence);

    let sum_error = weighted_outcome_sum(&[large, large], &[1.0, 1.0])
        .expect_err("finite accumulated sum beyond Stata numeric range must fail");
    assert_eq!(sum_error.code, ErrorCode::NonConvergence);

    let layout_error = weighted_outcome_sum(&[1.0], &[1.0, 1.0])
        .expect_err("misaligned weighted-outcome inputs must remain structural");
    assert_ne!(layout_error.code, ErrorCode::NonConvergence);
}

#[test]
fn scientific_and_structural_shadow_representation_failures_are_distinct() {
    let values = [STATA_SYSTEM_MISSING];
    let scientific = validate_stata_cells(
        "scientific fixture",
        &values,
        &[],
        StataCellClass::Scientific,
    )
    .expect_err("scientific shadow overflow must fail");
    assert_eq!(scientific.code, ErrorCode::NonConvergence);

    let structural = validate_stata_cells(
        "structural fixture",
        &values,
        &[],
        StataCellClass::Structural,
    )
    .expect_err("structural shadow overflow must fail");
    assert_ne!(structural.code, ErrorCode::NonConvergence);
}

#[test]
fn final_publication_peak_must_stay_within_the_preflight_phase_model() {
    let mut runtime = valid_runtime_skeleton();
    runtime[94] = 100.0;
    let error = finalize_ppml_runtime_shadow(
        &mut runtime,
        PpmlRuntimeFinalObservation {
            total_seconds: 0.5,
            publication_extension_seconds: 0.1,
            publication_peak_bytes: Some(101),
            action_start: None,
            action_end: None,
            previous_baseline_bytes: None,
            modeled_selected_bytes: 0,
        },
    )
    .expect_err("observed phase peak above the preflight model must fail");
    assert_eq!(error.code, ErrorCode::Internal);
}

#[test]
fn bounded_ledger_digests_bind_fields_order_and_every_generation_hash() {
    let report = ledger_digest_fixture();
    let trace = separation_trace_digest(&report).expect("trace digest");
    let hashes = generation_hash_digest(&report).expect("hash-sequence digest");

    let mut field_mutation = report.clone();
    field_mutation.methods[1].iterations += 1;
    assert_ne!(
        separation_trace_digest(&field_mutation).expect("field mutation"),
        trace
    );

    let mut order_mutation = report.clone();
    order_mutation.methods.swap(0, 1);
    assert_ne!(
        separation_trace_digest(&order_mutation).expect("order mutation"),
        trace
    );

    let mut hash_mutation = report.clone();
    hash_mutation.generation_hashes[1] ^= 1;
    assert_ne!(
        generation_hash_digest(&hash_mutation).expect("hash mutation"),
        hashes
    );

    let mut dual_proof = report.clone();
    dual_proof.methods[1]
        .certificate
        .as_mut()
        .expect("simplex certificate")
        .proof = SeparationCertificateProof::SimplexDualInfeasibility {
        boundary_rows: 3,
        nullity: 2,
    };
    let dual_trace = separation_trace_digest(&dual_proof).expect("simplex dual trace");
    assert_ne!(dual_trace, trace, "the dual proof tag is part of the trace");
    dual_proof.methods[1]
        .certificate
        .as_mut()
        .expect("simplex dual certificate")
        .proof = SeparationCertificateProof::SimplexDualInfeasibility {
        boundary_rows: 4,
        nullity: 2,
    };
    assert_ne!(
        separation_trace_digest(&dual_proof).expect("mutated simplex dual trace"),
        dual_trace,
        "simplex dual dimensions are canonical digest identity"
    );

    let mut process_only = report;
    process_only.methods[0].elapsed_seconds = 999.0;
    process_only.methods[1].note = "different diagnostic prose".into();
    assert_eq!(
        separation_trace_digest(&process_only).expect("process-only mutation"),
        trace,
        "timing and prose are not deterministic ledger identity"
    );
}

#[test]
fn separation_trace_binds_stable_exact_fe_term_and_original_row_identity() {
    use fereg_core::ppml_separation::{ExactFeLevelIdentity, ExactFeSeparationCertificate};
    use fereg_core::AbsorbTermId;

    let mut report = ledger_digest_fixture();
    report.methods[0].exact_fe_certificate = Some(ExactFeSeparationCertificate {
        checked_intercept_effects: 1,
        zero_only_levels: vec![ExactFeLevelIdentity {
            effect: AbsorbTermId::new(7).expect("stable absorbed term"),
            representative_original_row: 103,
        }],
        passed: true,
    });
    let trace = separation_trace_digest(&report).expect("stable exact-FE trace");

    let mut reused_compact_code = report.clone();
    reused_compact_code.methods[0]
        .exact_fe_certificate
        .as_mut()
        .expect("exact-FE certificate")
        .zero_only_levels[0]
        .representative_original_row = 104;
    assert_ne!(
        separation_trace_digest(&reused_compact_code).expect("mutated original-row anchor"),
        trace,
        "a reused detector-local level code cannot preserve public evidence identity",
    );

    let mut different_interaction = report;
    different_interaction.methods[0]
        .exact_fe_certificate
        .as_mut()
        .expect("exact-FE certificate")
        .zero_only_levels[0]
        .effect = AbsorbTermId::new(8).expect("different absorbed term");
    assert_ne!(
        separation_trace_digest(&different_interaction).expect("mutated absorbed term"),
        trace,
        "interaction term identity is part of the stable witness",
    );
}

#[test]
fn bounded_sequence_preserves_requested_methods_and_trace_positions() {
    let report = ledger_digest_fixture();
    let evidence = build_separation_sequence_evidence(
        &report,
        SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: false,
            mu: false,
        },
        true,
    )
    .expect("sequence evidence");
    assert_eq!(
        evidence.len(),
        SEPARATION_SEQUENCE_ROWS * SEPARATION_SEQUENCE_COLUMNS
    );
    assert_eq!(
        &evidence[..SEPARATION_SEQUENCE_COLUMNS],
        &[1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 4.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0]
    );
    let simplex = 2 * SEPARATION_SEQUENCE_COLUMNS;
    assert_eq!(
        &evidence[simplex..simplex + SEPARATION_SEQUENCE_COLUMNS],
        &[3.0, 1.0, 2.0, 1.0, 2.0, 2.0, 4.0, 3.0, 1.0, 0.0, 2.0, 2.0, 1.0]
    );
    let fixed_effects = SEPARATION_SEQUENCE_COLUMNS;
    assert_eq!(evidence[fixed_effects + 1], 0.0);
    assert_eq!(evidence[fixed_effects + 2], 0.0);
    assert_eq!(evidence[fixed_effects + 12], 1.0);
}

#[test]
fn inference_evidence_preserves_multiway_psd_repair_diagnostics() {
    let dof = PpmlDofReport {
        effective_observations: 20,
        sandwich_rank: 2,
        finite_within_rank: 2,
        absorbed_rank: 0,
        model_rank: 2,
        structural_residual: 18,
        reported: 2,
        minimum_cluster_groups: Some(3),
        connected_components: 0,
        absorbed_rank_is_approximate: false,
    };
    let mut vce = PpmlVceResult {
        requested: PpmlVceRequest::Clustered,
        effective: PpmlVceKind::Clustered,
        covariance: Vec::new(),
        standard_errors: Vec::new(),
        z_statistics: Vec::new(),
        coefficient_distribution: PpmlTestDistribution::StandardNormal,
        correction: 1.5,
        cluster_dimension_groups: vec![3, 4],
        cluster_subsets: Vec::new(),
        psd: fereg_core::ppml::PpmlPsdReport {
            state: PpmlPsdState::MultiwayClipped,
            adjusted: true,
            clipped_eigenvalues: 1,
            raw_minimum_eigenvalue: -0.25,
            final_minimum_eigenvalue: 0.0,
            final_rank: 1,
            eigensystem_reconstruction_relative_error: 1.0e-12,
            orthogonality_relative_error: 2.0e-12,
            clipped_reconstruction_relative_error: 3.0e-12,
            passed: true,
        },
        certificate: fereg_core::ppml::PpmlInferenceCertificate {
            bread_symmetry_relative_infinity_norm: 0.0,
            meat_relative_infinity_norm: 0.0,
            covariance_relative_infinity_norm: 0.0,
            covariance_symmetry_relative_infinity_norm: 0.0,
            wald_solve_relative_infinity_norm: 0.0,
            tolerance: 1.0e-8,
            passed: true,
        },
    };
    let evidence = encode_inference_evidence(&vce, &dof, 2).expect("PSD evidence");
    assert_eq!(evidence.len(), INFERENCE_ROWS);
    assert_eq!(&evidence[..8], &[3.0, 3.0, 1.5, 2.0, 1.0, 3.0, 1.0, 1.0]);
    assert_eq!(evidence[8], -0.25);
    assert_eq!(evidence[9], 0.0);
    assert_eq!(evidence[10], 1.0);
    assert_eq!(evidence[14], 1.0e-8);
    assert_eq!(evidence[15], 1.0);

    vce.psd.state = PpmlPsdState::MultiwayRaw;
    assert_eq!(
        encode_inference_evidence(&vce, &dof, 2)
            .expect_err("inconsistent repair state")
            .code,
        ErrorCode::Internal
    );
}
