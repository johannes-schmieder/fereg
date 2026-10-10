use super::*;
use crate::data::DenseMatrix;
use crate::options::MemoryMode;
use crate::ppml::{
    compute_ppml_inference, plan_ppml_memory, PpmlInferenceInput, PpmlMemoryConfig,
    PpmlMemoryDimensions, PpmlSolverEngine, PpmlSolverFamily, PpmlSolverPlan, PpmlVceRequest,
};

#[allow(clippy::too_many_arguments)]
fn output_state_for_test(
    original_observations: usize,
    original_rows: impl Into<Arc<Vec<usize>>>,
    offset: Vec<f64>,
    finite_predictor: Vec<f64>,
    absorbed_predictor: Vec<f64>,
    eta: Vec<f64>,
    mean: Vec<f64>,
    saved_effects: Option<PpmlSavedEffectOutput>,
    tolerance: f64,
) -> Result<PpmlOutputState> {
    PpmlOutputState::new_requested(
        original_observations,
        original_rows.into(),
        &offset,
        &finite_predictor,
        &absorbed_predictor,
        &eta,
        &mean,
        &[],
        saved_effects,
        super::super::PreparedPpmlOutputRequest {
            xb: true,
            absorbed: true,
            eta: true,
            mean: true,
            requested_observation_columns: 4,
            ..Default::default()
        },
        tolerance,
    )
}

fn exact_state(saved: bool) -> PpmlOutputState {
    let offset = vec![0.1_f64, -0.2, 0.3];
    let finite = vec![0.2_f64, 0.4, -0.1];
    let first = vec![0.3_f64, -0.1, 0.2];
    let second = vec![-0.05_f64, 0.2, 0.1];
    let absorbed = first
        .iter()
        .zip(&second)
        .map(|(left, right)| left + right)
        .collect::<Vec<_>>();
    let eta = offset
        .iter()
        .zip(&finite)
        .zip(&absorbed)
        .map(|((offset, finite), absorbed)| offset + finite + absorbed)
        .collect::<Vec<_>>();
    let mean = eta.iter().map(|value| value.exp()).collect::<Vec<_>>();
    let effects = saved.then_some(PpmlSavedEffectOutput {
        channels: vec![first, second],
        declared_channels: 2,
        declared_channel_indices: vec![0, 1],
        total_reconstruction_relative_infinity_norm: 0.0,
        full_decomposition_relative_infinity_norm: 0.0,
        full_decomposition_tolerance: 1e-12,
        full_decomposition_certified: true,
    });
    output_state_for_test(
        5,
        vec![0, 2, 4],
        offset,
        finite,
        absorbed,
        eta,
        mean,
        effects,
        1e-12,
    )
    .expect("certified output state")
}

fn certified_composite_input() -> CertifiedPpmlResultInput {
    let outcome = vec![1.0_f64, 3.0, 2.0, 4.0];
    let prior_weights = vec![1.0_f64; outcome.len()];
    let mean_value = 2.5_f64;
    let eta_value = mean_value.ln();
    let mean = vec![eta_value.exp(); outcome.len()];
    let eta = vec![eta_value; outcome.len()];
    let coefficients = PpmlCoefficientOutput::new(
        vec![eta_value],
        vec![PpmlCoefficientStatus::Identified],
        vec![Some(0)],
        vec![0],
    )
    .expect("coefficient result");
    let sample = PpmlFinalSample::new(
        outcome.len(),
        outcome.len(),
        Arc::new((0..outcome.len()).collect::<Vec<_>>()),
        (0..outcome.len()).collect::<Vec<_>>(),
        vec![DropReason::Retained; outcome.len()],
        outcome.clone(),
        prior_weights.clone(),
    )
    .expect("final sample");
    let weight_report = PreparedPpmlWeightReport {
        kind: None,
        physical_observations: outcome.len(),
        effective_observations: outcome.len(),
        raw_weight_sum: None,
        log_raw_weight_sum: None,
        prepared_weight_sum: outcome.len() as f64,
        raw_to_prepared_multiplier: Some(1.0),
        log_raw_to_prepared_multiplier: 0.0,
    };
    let design =
        DenseMatrix::from_columns(&[vec![1.0_f64; outcome.len()]]).expect("constant design");
    let score_residuals = outcome
        .iter()
        .zip(&mean)
        .map(|(outcome, mean)| outcome - mean)
        .collect::<Vec<_>>();
    let inference = compute_ppml_inference(PpmlInferenceInput {
        coefficients: coefficients.values(),
        full_dimension: coefficients.values().len(),
        within_design: &design,
        reported_finite_rank: 1,
        score_residuals: &score_residuals,
        prior_weights: &prior_weights,
        weight_report,
        identified_bread: &[0.1],
        kept_full_slots: coefficients.kept_full_slots(),
        absorbed_rank: 0,
        connected_components: 0,
        absorbed_rank_is_approximate: false,
        clusters: &[],
        requested_vce: PpmlVceRequest::Robust,
        wald_columns: &[],
        wald_reference: None,
        certificate_tolerance: 1e-10,
    })
    .expect("certified inference");
    let output = output_state_for_test(
        outcome.len(),
        (0..outcome.len()).collect::<Vec<_>>(),
        vec![0.0; outcome.len()],
        eta.clone(),
        vec![0.0; outcome.len()],
        eta,
        mean.clone(),
        None,
        1e-12,
    )
    .expect("output state");
    let objective = poisson_objective(&outcome, &mean, &prior_weights).expect("objective");
    let separation = SeparationReport {
        candidate_observations: outcome.len(),
        active: vec![true; outcome.len()],
        retained_observations: (0..outcome.len()).collect::<Vec<_>>(),
        dropped_observations: Vec::new(),
        drop_reasons: vec![None; outcome.len()],
        singleton_count: 0,
        separated_count: 0,
        generations: 0,
        generation_hashes: vec![candidate_ledger_hash_candidate_axis(
            &(0..outcome.len()).collect::<Vec<_>>(),
            &vec![true; outcome.len()],
            &vec![None; outcome.len()],
        )
        .expect("identity candidate ledger hash")],
        methods: Vec::new(),
    };
    let separation_config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let separation_seal = super::super::prepared_separation::certify_disabled_separation_for_test(
        separation_config,
        &separation,
        sample.original_rows(),
        sample.outcome(),
    )
    .expect("disabled separation seal");
    let dimensions = PpmlMemoryDimensions {
        metadata: Default::default(),
        observations: outcome.len(),
        original_observations: outcome.len(),
        candidate_observations: outcome.len(),
        loaded_numeric_columns: 1,
        retained_base_adjustment_capacity_elements: 0,
        retained_raw_preseparation_probability_weight_elements: 0,
        finite_columns: 1,
        coefficient_slots: 1,
        reported_coefficient_slots: 1,
        fe_dimensions: 0,
        fe_total_levels: 0,
        fe_max_levels: 0,
        fe_total_channels: 0,
        fe_max_channels: 0,
        fe_max_level_members: 0,
        fe_coefficient_directions: 0,
        fe_max_effect_directions: 0,
        cluster_dimensions: 0,
        cluster_subset_groups: 0,
        separation_workspace_vectors: 0,
        separation_singleton_enabled: false,
        separation_method_count: 0,
        separation_generation_capacity: 1,
        separation_rebase_enabled: false,
        separation_exact_fe_enabled: false,
        separation_mu_enabled: false,
        simplex_tableau_cells: 0,
        requested_output_columns: 0,
        requested_mean_output: false,
        requested_saved_fe_channels: 0,
        publication_matrix_cells: 64,
        threads: 1,
        allocation_audit_enabled: false,
    };
    let memory_plan = plan_ppml_memory(
        dimensions,
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("memory plan");
    let workspace = PpmlIterationWorkspace::from_initial_state(
        memory_plan,
        vec![0.0; outcome.len()],
        vec![0.0; memory_plan.dimensions.coefficient_slots],
    )
    .expect("memory fixture workspace");
    let mut memory = PpmlMemoryTelemetry::new(memory_plan, 0, 1).expect("memory telemetry");
    memory.observe_linear_route(FeSolverKind::Map);
    memory.observe_scratch(0);
    memory
        .observe_diagnostic_records(0)
        .expect("memory diagnostic telemetry");
    memory
        .certify_workspace(&workspace)
        .expect("memory workspace telemetry");
    CertifiedPpmlResultInput {
        bias: None,
        coefficients,
        sample,
        weights: weight_report,
        adjustment: PreparedPpmlAdjustmentReport {
            input_kind: super::super::PreparedPpmlAdjustmentKind::None,
            prepared_as_offset: false,
            exposure_log_transformed: false,
        },
        objective,
        inference,
        output,
        convergence: PpmlConvergenceReport {
            converged: true,
            two_consecutive_convergence: true,
            rank_frozen: true,
            outer_iterations: 2,
            inner_iterations: 2,
            step_halvings: 0,
            strict_profile_rejections: 0,
            strict_final_solve_passed: true,
            linear_within_relative_infinity_norm: 0.0,
            linear_reconstruction_relative_infinity_norm: 0.0,
            final_score: PpmlCertificate {
                finite_absolute_infinity_norm: 0.0,
                fixed_effect_absolute_infinity_norm: 0.0,
                relative_infinity_norm: 0.0,
                relative_two_norm: 0.0,
                passed: true,
            },
        },
        fixed_effects: Vec::new(),
        clusters: Vec::new(),
        separation,
        separation_config,
        separation_seal,
        memory,
        solver: PpmlSolverReport::new(
            PpmlSolverPlan::new(PpmlSolverFamily::Auto),
            PpmlSolverEngine::Map,
        ),
        certificate_tolerance: 1e-10,
    }
}

fn certified_clustered_composite_input() -> CertifiedPpmlResultInput {
    let mut input = certified_composite_input();
    let design = DenseMatrix::from_columns(&[vec![1.0_f64; input.sample.outcome().len()]])
        .expect("constant design");
    let score_residuals = input
        .sample
        .outcome()
        .iter()
        .zip(input.output.mean())
        .map(|(outcome, mean)| outcome - mean)
        .collect::<Vec<_>>();
    let cluster = [0_u32, 0, 1, 1];
    input.inference = compute_ppml_inference(PpmlInferenceInput {
        coefficients: input.coefficients.values(),
        full_dimension: input.coefficients.values().len(),
        within_design: &design,
        reported_finite_rank: 1,
        score_residuals: &score_residuals,
        prior_weights: input.sample.prior_weights(),
        weight_report: input.weights,
        identified_bread: &[0.1],
        kept_full_slots: input.coefficients.kept_full_slots(),
        absorbed_rank: 0,
        connected_components: 0,
        absorbed_rank_is_approximate: false,
        clusters: &[&cluster],
        requested_vce: PpmlVceRequest::Clustered,
        wald_columns: &[],
        wald_reference: None,
        certificate_tolerance: input.certificate_tolerance,
    })
    .expect("clustered inference");
    input.clusters = vec![PpmlClusterDimensionResult {
        declared_dimension: 0,
        label: "g".to_owned(),
        source_ids: vec![1],
        groups: 2,
    }];
    let mut dimensions = input.memory.plan.dimensions;
    dimensions.cluster_dimensions = 1;
    dimensions.cluster_subset_groups = 2;
    let memory_plan = plan_ppml_memory(
        dimensions,
        PpmlMemoryConfig {
            requested: MemoryMode::Fast,
            ..PpmlMemoryConfig::default()
        },
    )
    .expect("clustered memory plan");
    let workspace = PpmlIterationWorkspace::from_initial_state(
        memory_plan,
        vec![0.0; dimensions.observations],
        vec![0.0; memory_plan.dimensions.coefficient_slots],
    )
    .expect("clustered memory fixture workspace");
    input.memory = PpmlMemoryTelemetry::new(memory_plan, 0, 1).expect("clustered memory telemetry");
    input.memory.observe_linear_route(FeSolverKind::Map);
    input.memory.observe_scratch(0);
    input
        .memory
        .observe_diagnostic_records(0)
        .expect("clustered memory diagnostic telemetry");
    input
        .memory
        .certify_workspace(&workspace)
        .expect("clustered memory workspace telemetry");
    input
}

#[test]
fn exact_predictor_and_saved_channels_certify() {
    let state = exact_state(true);
    assert!(state.certificate().passed);
    assert_eq!(state.certificate().saved_channels, 2);
    assert_eq!(
        state
            .saved_effects()
            .expect("saved")
            .channels
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![3, 3]
    );
}

#[test]
fn xb_includes_offset_but_not_absorbed_contribution() {
    let state = exact_state(false);
    let xb = state.prediction(PpmlPredictionKind::Xb, None).expect("xb");
    for (actual, expected) in xb.iter().zip([0.3_f64, 0.2, 0.2]) {
        assert!((actual - expected).abs() <= 2e-16);
    }
    assert_eq!(
        state
            .prediction(PpmlPredictionKind::Absorbed, None)
            .expect("absorbed"),
        state.absorbed_predictor()
    );
    assert_eq!(
        state
            .prediction(PpmlPredictionKind::Eta, None)
            .expect("eta"),
        state.eta()
    );
}

#[test]
fn residual_predictions_require_outcome_and_remain_unweighted() {
    let state = exact_state(false);
    assert_eq!(
        state
            .prediction(PpmlPredictionKind::Response, None)
            .expect_err("missing outcome")
            .code,
        ErrorCode::InvalidArgument
    );
    let outcome = [1.0_f64, 2.0, 3.0];
    let response = state
        .prediction(PpmlPredictionKind::Response, Some(&outcome))
        .expect("response");
    assert_eq!(
        response,
        outcome
            .iter()
            .zip(state.mean())
            .map(|(outcome, mean)| outcome - mean)
            .collect::<Vec<_>>()
    );
}

#[test]
fn scatter_uses_stable_original_rows_and_missing_elsewhere() {
    let state = exact_state(false);
    let scattered = state
        .scatter_to_original(&[10.0, 20.0, 30.0], f64::NAN)
        .expect("scatter");
    assert_eq!(scattered[0], 10.0);
    assert!(scattered[1].is_nan());
    assert_eq!(scattered[2], 20.0);
    assert!(scattered[3].is_nan());
    assert_eq!(scattered[4], 30.0);
}

#[test]
fn nonmonotone_mapping_and_inconsistent_saved_effects_fail_closed() {
    let state = exact_state(true);
    let error = output_state_for_test(
        5,
        vec![0, 0, 4],
        vec![0.0; state.xb.len()],
        state.xb.clone(),
        state.absorbed_predictor.clone(),
        state.eta.clone(),
        state.mean.clone(),
        state.saved_effects.clone(),
        1e-12,
    )
    .expect_err("mapping must be unique and ordered");
    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(error.return_code(), 3499);

    let mut saved = state.saved_effects.clone().expect("saved");
    saved.channels[0][1] += 0.25;
    let error = output_state_for_test(
        5,
        vec![0, 2, 4],
        vec![0.0; state.xb.len()],
        state.xb,
        state.absorbed_predictor,
        state.eta,
        state.mean,
        Some(saved),
        1e-12,
    )
    .expect_err("saved channels must reconstruct");
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert_eq!(error.return_code(), 430);
}

#[test]
fn partial_saved_mask_uses_full_decomposition_evidence_without_public_sum() {
    let state = exact_state(true);
    let complete = state.saved_effects.clone().expect("complete saved effects");
    let partial = PpmlSavedEffectOutput {
        channels: vec![complete.channels[0].clone()],
        declared_channels: 2,
        declared_channel_indices: vec![0],
        total_reconstruction_relative_infinity_norm: 0.0,
        full_decomposition_relative_infinity_norm: 0.0,
        full_decomposition_tolerance: 1e-12,
        full_decomposition_certified: true,
    };
    let partial_state = output_state_for_test(
        state.original_observations,
        state.original_rows,
        vec![0.0; state.xb.len()],
        state.xb,
        state.absorbed_predictor,
        state.eta,
        state.mean,
        Some(partial),
        1e-12,
    )
    .expect("partial saved mask uses complete recovery evidence");
    assert_eq!(partial_state.certificate().saved_channels, 1);
    assert_eq!(partial_state.certificate().saved_declared_channels, 2);
    assert!(!partial_state.certificate().saved_complete_decomposition);
}

#[test]
fn duplicate_or_out_of_range_saved_declared_indices_fail_closed() {
    let state = exact_state(true);
    let mut duplicate = state.saved_effects.clone().expect("saved effects");
    duplicate.declared_channel_indices = vec![0, 0];
    assert_eq!(
        output_state_for_test(
            state.original_observations,
            state.original_rows.clone(),
            vec![0.0; state.xb.len()],
            state.xb.clone(),
            state.absorbed_predictor.clone(),
            state.eta.clone(),
            state.mean.clone(),
            Some(duplicate),
            1e-12,
        )
        .expect_err("duplicate saved declared identity")
        .code,
        ErrorCode::Internal
    );

    let mut out_of_range = state.saved_effects.clone().expect("saved effects");
    out_of_range.declared_channel_indices = vec![0, 2];
    assert_eq!(
        output_state_for_test(
            state.original_observations,
            state.original_rows,
            vec![0.0; state.xb.len()],
            state.xb,
            state.absorbed_predictor,
            state.eta,
            state.mean,
            Some(out_of_range),
            1e-12,
        )
        .expect_err("out-of-range saved declared identity")
        .code,
        ErrorCode::Internal
    );
}

#[test]
fn nonconstant_mu_requires_final_hessian_not_preparation_weights() {
    let prior = [1.0_f64, 1.0, 2.0];
    let mean = [1.0_f64, 3.0, 0.5];
    let q_only = [1.0_f64, 1.0, 2.0];
    assert_eq!(
        validate_ppml_hessian_weights(BorrowedMapWeights::Provided(&q_only), &prior, &mean)
            .expect_err("q alone is not the PPML recovery metric")
            .code,
        ErrorCode::Internal
    );
    let final_hessian = [1.0_f64, 3.0, 1.0];
    validate_ppml_hessian_weights(BorrowedMapWeights::Provided(&final_hessian), &prior, &mean)
        .expect("q*mu is the certified recovery metric");
}

#[test]
fn null_likelihood_is_constant_only_and_pseudo_r_squared_uses_it() {
    let outcome = [1.0_f64, 3.0, 2.0, 6.0];
    let weights = [1.0_f64; 4];
    let fitted_mean = [2.0_f64, 2.0, 4.0, 4.0];
    let objective = super::super::poisson_objective(&outcome, &fitted_mean, &weights)
        .expect("fitted objective");
    let null_objective =
        crate::ppml::poisson_null_objective(&outcome, &weights).expect("null objective");
    let statistics = PpmlLikelihoodStatistics::from_replayed_null(objective, null_objective)
        .expect("statistics");
    assert!((statistics.null_mean - 3.0).abs() <= 8.0 * f64::EPSILON);
    let null = super::super::poisson_objective(&outcome, &[3.0; 4], &weights)
        .expect("explicit null objective");
    assert!(
        (statistics.null_log_likelihood - null.log_likelihood).abs()
            <= 8.0 * f64::EPSILON * null.log_likelihood.abs().max(1.0)
    );
    let expected = 1.0 - objective.log_likelihood / null.log_likelihood;
    assert!((statistics.pseudo_r_squared - expected).abs() <= 8.0 * f64::EPSILON);
}

#[test]
fn null_likelihood_does_not_accept_an_offset_or_fixed_effect_input() {
    // The API has only outcome and prior weights by construction. This
    // fixture also verifies literal fweight replication.
    let compact_y = [1.0_f64, 4.0];
    let compact_weights = [2.0_f64, 3.0];
    let compact =
        super::super::poisson_null_objective(&compact_y, &compact_weights).expect("compact null");
    let expanded_y = [1.0_f64, 1.0, 4.0, 4.0, 4.0];
    let expanded =
        super::super::poisson_null_objective(&expanded_y, &[1.0; 5]).expect("expanded null");
    assert!((compact.mean - expanded.mean).abs() < 1e-15);
    assert!((compact.log_likelihood - expanded.log_likelihood).abs() < 1e-13);
}

#[test]
fn final_sample_ordered_membership_matches_explicit_mask_oracle() {
    for n in 1..=6 {
        for rows_mask in 1_usize..(1 << n) {
            for ledger_mask in 0_usize..(1 << n) {
                let rows: Vec<usize> = (0..n).filter(|row| rows_mask & (1 << row) != 0).collect();
                let reasons: Vec<DropReason> = (0..n)
                    .map(|row| {
                        if ledger_mask & (1 << row) != 0 {
                            DropReason::Retained
                        } else {
                            [
                                DropReason::ExcludedByIfIn,
                                DropReason::MissingCluster,
                                DropReason::Singleton,
                            ][row % 3]
                        }
                    })
                    .collect();
                let candidates = reasons
                    .iter()
                    .filter(|r| **r != DropReason::ExcludedByIfIn)
                    .count();
                let mut explicit_mask = vec![false; n];
                for &row in &rows {
                    explicit_mask[row] = true;
                }
                let matches = reasons
                    .iter()
                    .enumerate()
                    .all(|(row, reason)| explicit_mask[row] == (*reason == DropReason::Retained));
                let retained = rows.len();
                let result = PpmlFinalSample::new(
                    n,
                    candidates,
                    Arc::new((0..n).collect()),
                    rows,
                    reasons,
                    vec![1.0; retained],
                    vec![1.0; retained],
                );
                assert_eq!(
                    result.is_ok(),
                    matches,
                    "n={n}, rows={rows_mask}, ledger={ledger_mask}"
                );
            }
        }
    }
}

#[test]
fn composite_result_is_one_certified_prediction_and_inference_authority() {
    let result = CertifiedPpmlResult::new(certified_composite_input()).expect("composite result");
    assert!(result.certificate().passed);
    assert_eq!(result.inference().dof.structural_residual, 3);
    assert_eq!(result.inference().dof.reported, 3);
    assert_eq!(
        result.inference().vce.coefficient_distribution,
        super::super::PpmlTestDistribution::StandardNormal
    );
    assert_eq!(
        result.inference().wald.distribution,
        super::super::PpmlTestDistribution::ChiSquared
    );
    assert!(result.inference().vce.z_statistics[0].is_finite());
    assert_eq!(
        result.prediction(PpmlPredictionKind::Mu).expect("mean"),
        result.output().mean()
    );
    assert_eq!(
        result
            .prediction(PpmlPredictionKind::Response)
            .expect("response residual"),
        result
            .sample()
            .outcome()
            .iter()
            .zip(result.output().mean())
            .map(|(outcome, mean)| outcome - mean)
            .collect::<Vec<_>>()
    );
}

#[test]
fn composite_result_rejects_cross_generation_rows_and_objective_drift() {
    let mut generation = certified_composite_input();
    generation.separation.retained_observations.swap(1, 2);
    assert_eq!(
        CertifiedPpmlResult::new(generation)
            .expect_err("cross-generation row order")
            .code,
        ErrorCode::Internal
    );

    let mut objective = certified_composite_input();
    objective.objective.deviance += 0.25;
    assert_eq!(
        CertifiedPpmlResult::new(objective)
            .expect_err("objective drift")
            .code,
        ErrorCode::NonConvergence
    );
}

#[test]
fn frozen_memory_route_axes_remain_bound_to_the_certified_final_sample() {
    let mut original = certified_composite_input();
    original.sample.original_observations += 1;
    let error = CertifiedPpmlResult::new(original)
        .expect_err("memory route must retain the certified original axis");
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.message.contains("frozen memory-route axes"));

    let mut candidate = certified_composite_input();
    candidate.sample.candidate_observations -= 1;
    let error = CertifiedPpmlResult::new(candidate)
        .expect_err("memory route must retain the certified if/in candidate axis");
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.message.contains("frozen memory-route axes"));

    let mut generation_zero = certified_composite_input();
    Arc::make_mut(&mut generation_zero.sample.prepared_original_rows).pop();
    let error = CertifiedPpmlResult::new(generation_zero)
        .expect_err("memory route must retain the certified generation-zero active axis");
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.message.contains("frozen memory-route axes"));

    let mut uncharged_probability_weights = certified_composite_input();
    let prepared_rows = uncharged_probability_weights
        .sample
        .prepared_original_rows()
        .len();
    uncharged_probability_weights.weights.kind = Some(WeightKind::Probability);
    uncharged_probability_weights.weights.raw_weight_sum = Some(prepared_rows as f64);
    uncharged_probability_weights.weights.log_raw_weight_sum = Some((prepared_rows as f64).ln());
    let error = CertifiedPpmlResult::new(uncharged_probability_weights)
        .expect_err("pweights require one retained raw value per generation-zero active row");
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.message.contains("retained pre-separation"));

    let mut spurious_unweighted_capacity = certified_composite_input();
    spurious_unweighted_capacity
        .memory
        .plan
        .preflight_dimensions
        .retained_raw_preseparation_probability_weight_elements = prepared_rows;
    let error = validate_memory_sample_axes(
        &spurious_unweighted_capacity.sample,
        spurious_unweighted_capacity.weights,
        spurious_unweighted_capacity.adjustment,
        spurious_unweighted_capacity.separation_config,
        &spurious_unweighted_capacity.memory,
    )
    .expect_err("unweighted fits cannot claim retained raw-pweight capacity");
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.message.contains("retained pre-separation"));

    let mut uncharged_offset = certified_composite_input();
    uncharged_offset.adjustment = PreparedPpmlAdjustmentReport {
        input_kind: PreparedPpmlAdjustmentKind::Offset,
        prepared_as_offset: true,
        exposure_log_transformed: false,
    };
    let error = CertifiedPpmlResult::new(uncharged_offset)
        .expect_err("offset fits require immutable candidate-capacity owner storage");
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.message.contains("retained pre-separation"));

    let mut spurious_adjustment_capacity = certified_composite_input();
    let candidate_rows = spurious_adjustment_capacity.sample.candidate_observations();
    spurious_adjustment_capacity
        .memory
        .plan
        .dimensions
        .retained_base_adjustment_capacity_elements = candidate_rows;
    spurious_adjustment_capacity
        .memory
        .plan
        .preflight_dimensions
        .retained_base_adjustment_capacity_elements = candidate_rows;
    let error = validate_memory_sample_axes(
        &spurious_adjustment_capacity.sample,
        spurious_adjustment_capacity.weights,
        spurious_adjustment_capacity.adjustment,
        spurious_adjustment_capacity.separation_config,
        &spurious_adjustment_capacity.memory,
    )
    .expect_err("unadjusted fits cannot claim retained base-adjustment capacity");
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.message.contains("retained pre-separation"));

    let mut active = certified_composite_input();
    Arc::make_mut(&mut active.sample.original_rows).pop();
    active.sample.outcome.pop();
    active.sample.prior_weights.pop();
    active.weights.physical_observations -= 1;
    active.weights.effective_observations -= 1;
    active.weights.prepared_weight_sum -= 1.0;
    let error = CertifiedPpmlResult::new(active)
        .expect_err("memory route must retain the certified active axis");
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.message.contains("frozen memory-route axes"));
}

#[test]
fn composite_result_rejects_mutated_dof_vce_wald_certificate_and_memory_families() {
    let mut dof = certified_composite_input();
    dof.inference.dof.reported -= 1;
    assert_eq!(
        CertifiedPpmlResult::new(dof)
            .expect_err("DoF identity")
            .code,
        ErrorCode::Internal
    );

    let mut vce = certified_composite_input();
    vce.inference.vce.effective = PpmlVceKind::Clustered;
    assert_eq!(
        CertifiedPpmlResult::new(vce)
            .expect_err("VCE identity")
            .code,
        ErrorCode::Internal
    );

    let mut wald = certified_composite_input();
    wald.inference.wald.distribution = PpmlTestDistribution::StandardNormal;
    assert_eq!(
        CertifiedPpmlResult::new(wald)
            .expect_err("Wald distribution identity")
            .code,
        ErrorCode::Internal
    );

    let mut reference = certified_composite_input();
    reference.inference.wald.reference = Some(super::super::PpmlWaldReference {
        slot: 1,
        loadings: Vec::new(),
        span_error: 0.0,
        span_tolerance: 1e-12,
    });
    assert_eq!(
        CertifiedPpmlResult::new(reference)
            .expect_err("constant reference must identify a kept slot")
            .code,
        ErrorCode::InvalidArgument
    );

    let mut covariance = certified_composite_input();
    covariance.inference.vce.standard_errors[0] *= 2.0;
    assert_eq!(
        CertifiedPpmlResult::new(covariance)
            .expect_err("covariance/SE certificate")
            .code,
        ErrorCode::NonConvergence
    );

    let mut certificate = certified_composite_input();
    certificate.inference.vce.certificate.passed = false;
    assert_eq!(
        CertifiedPpmlResult::new(certificate)
            .expect_err("VCE certificate")
            .code,
        ErrorCode::NonConvergence
    );

    let mut convergence = certified_composite_input();
    convergence.convergence.linear_within_relative_infinity_norm = -1.0;
    assert_eq!(
        CertifiedPpmlResult::new(convergence)
            .expect_err("convergence certificate")
            .code,
        ErrorCode::NonConvergence
    );

    let mut adjustment = certified_composite_input();
    adjustment.adjustment.prepared_as_offset = true;
    assert_eq!(
        CertifiedPpmlResult::new(adjustment)
            .expect_err("adjustment identity")
            .code,
        ErrorCode::Internal
    );

    let mut memory = certified_composite_input();
    memory.memory.plan.dimensions.observations += 1;
    assert_eq!(
        CertifiedPpmlResult::new(memory)
            .expect_err("memory identity")
            .code,
        ErrorCode::Internal
    );

    let mut generated = certified_composite_input();
    generated.coefficients.generated_columns[0] = None;
    assert_eq!(
        CertifiedPpmlResult::new(generated)
            .expect_err("generated-column identity")
            .code,
        ErrorCode::Internal
    );
}

#[test]
fn clustered_subset_identity_and_declared_metadata_fail_closed() {
    CertifiedPpmlResult::new(certified_clustered_composite_input())
        .expect("valid one-way clustered result");

    let mut subset = certified_clustered_composite_input();
    subset.inference.vce.cluster_subsets[0].sign = -1;
    assert_eq!(
        CertifiedPpmlResult::new(subset)
            .expect_err("cluster subset identity")
            .code,
        ErrorCode::Internal
    );

    let mut metadata = certified_clustered_composite_input();
    metadata.clusters[0].label.clear();
    assert_eq!(
        CertifiedPpmlResult::new(metadata)
            .expect_err("cluster declaration identity")
            .code,
        ErrorCode::Internal
    );
}

#[test]
fn separation_generation_sample_mapping_and_method_ranges_fail_closed() {
    let mut hash = certified_composite_input();
    hash.separation.generation_hashes[0] ^= 1;
    assert_eq!(
        CertifiedPpmlResult::new(hash)
            .expect_err("generation hash")
            .code,
        ErrorCode::Internal
    );

    let mut sample_reason = certified_composite_input();
    sample_reason.sample.drop_reasons[0] = DropReason::SimplexSeparation;
    assert_eq!(
        CertifiedPpmlResult::new(sample_reason)
            .expect_err("sample reason identity")
            .code,
        ErrorCode::Internal
    );

    let mut out_of_range = certified_composite_input().separation;
    out_of_range.methods.push(SeparationMethodReport {
        generation: 0,
        method: SeparationMethod::Singletons,
        status: MethodStatus::Certified,
        iterations: 1,
        dropped_observations: vec![out_of_range.candidate_observations],
        singleton_drops: Vec::new(),
        certificate: None,
        exact_fe_certificate: None,
        elapsed_seconds: 0.0,
        workspace_observation_vectors: 0,
        simplex_tableau_cells: 0,
        note: "adversarial out-of-range row".into(),
    });
    let candidate_original_rows = (0..out_of_range.candidate_observations).collect::<Vec<_>>();
    let disabled = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    assert!(validate_separation_report(
        &out_of_range,
        disabled,
        &candidate_original_rows,
        1,
        &[],
        0,
    )
    .is_err());
}

#[test]
fn separation_sorted_lists_still_require_a_disjoint_complete_partition() {
    let input = certified_composite_input();
    let original_rows = (0..input.separation.candidate_observations).collect::<Vec<_>>();
    let check = |report: &SeparationReport| {
        validate_separation_report(report, input.separation_config, &original_rows, 1, &[], 0)
    };
    assert!(check(&input.separation).unwrap());
    for overlap in [1, input.separation.candidate_observations - 1] {
        let mut report = input.separation.clone();
        // Both lists remain sorted and their lengths/counts agree, but row
        // zero is missing and another row belongs to both partitions.
        report.retained_observations.remove(0);
        report.dropped_observations.push(overlap);
        report.active[overlap] = false;
        report.drop_reasons[overlap] = Some(SeparationDropReason::InitialSingleton);
        report.singleton_count = 1;
        assert_eq!(check(&report).unwrap_err().code, ErrorCode::Internal);
    }
    let mut mask = input.separation.clone();
    mask.active[0] = false;
    assert_eq!(check(&mask).unwrap_err().code, ErrorCode::Internal);
}

#[test]
fn certified_result_binds_separation_none_to_its_exact_request_and_seal() {
    let mut default_substitution = certified_composite_input();
    assert!(default_substitution.separation.methods.is_empty());
    assert!(!default_substitution.separation_config.drop_singletons);
    assert_eq!(
        default_substitution.separation_config.methods,
        SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: false,
        }
    );
    default_substitution.separation_config = SeparationConfig::default();
    assert_eq!(
        CertifiedPpmlResult::new(default_substitution)
            .expect_err("an empty separation(none) report cannot impersonate the default chain")
            .code,
        ErrorCode::Internal
    );

    let mut report_mutation = certified_composite_input();
    report_mutation
        .separation
        .methods
        .push(SeparationMethodReport {
            generation: 0,
            method: SeparationMethod::IteratedRectifier,
            status: MethodStatus::NoSeparation,
            iterations: 1,
            dropped_observations: Vec::new(),
            singleton_drops: Vec::new(),
            certificate: Some(SeparationCertificate {
                proof:
                    crate::ppml_separation::SeparationCertificateProof::IteratedRectifierFixedPoint,
                interior_zero_relative: 0.0,
                boundary_violation_relative: 0.0,
                span_residual_relative: 0.0,
                negative_margin_relative: 0.0,
                direction_scale: 0.0,
                positive_maximum: 0.0,
                passed: true,
            }),
            exact_fe_certificate: None,
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 8,
            simplex_tableau_cells: 0,
            note: "self-minted fixed-point certificate".into(),
        });
    assert_eq!(
        CertifiedPpmlResult::new(report_mutation)
            .expect_err("a self-minted no-separation certificate lacks an independent replay seal")
            .code,
        ErrorCode::Internal
    );

    let mut generation_substitution = certified_composite_input();
    generation_substitution.sample.outcome[0] += 1.0;
    assert_eq!(
        CertifiedPpmlResult::new(generation_substitution)
            .expect_err("a replay seal cannot be transplanted onto another final outcome")
            .code,
        ErrorCode::Internal
    );
}

#[test]
fn raw_domain_and_positive_support_keep_their_public_return_codes() {
    let sample = |outcome: Vec<f64>, prior_weights: Vec<f64>| {
        PpmlFinalSample::new(
            outcome.len(),
            outcome.len(),
            Arc::new((0..outcome.len()).collect::<Vec<_>>()),
            (0..outcome.len()).collect::<Vec<_>>(),
            vec![DropReason::Retained; outcome.len()],
            outcome,
            prior_weights,
        )
    };

    let negative = sample(vec![-f64::MIN_POSITIVE], vec![1.0])
        .expect_err("negative outcome is outside the PPML domain");
    assert_eq!(negative.code, ErrorCode::InvalidData);
    assert_eq!(negative.return_code(), 459);

    let zero_weight = sample(vec![0.25], vec![0.0])
        .expect_err("nonpositive prior weight is outside the PPML domain");
    assert_eq!(zero_weight.code, ErrorCode::InvalidData);
    assert_eq!(zero_weight.return_code(), 459);

    let all_zero = sample(vec![0.0, 0.0], vec![1.0, 1.0])
        .expect_err("all-zero outcome has no estimable PPML support");
    assert_eq!(all_zero.code, ErrorCode::InsufficientObservations);
    assert_eq!(all_zero.return_code(), 2001);

    sample(vec![0.0, 0.25], vec![1.0, 1.0])
        .expect("finite nonnegative fractional outcomes are in-domain");
}

#[test]
fn separation_report_envelope_is_exact_for_all_policy_subsets() {
    let candidate_original_rows = vec![7_usize];
    let identity_hash =
        candidate_ledger_hash_candidate_axis(&candidate_original_rows, &[true], &[None])
            .expect("identity candidate ledger hash");
    for policy_bits in 0_u8..32 {
        let config = SeparationConfig {
            methods: SeparationMethods {
                fixed_effects: policy_bits & 0b0_0010 != 0,
                simplex: policy_bits & 0b0_0100 != 0,
                iterated_rectifier: policy_bits & 0b0_1000 != 0,
                mu: policy_bits & 0b1_0000 != 0,
            },
            drop_singletons: policy_bits & 0b0_0001 != 0,
            ..SeparationConfig::default()
        };
        let expected_policies = policy_bits.count_ones() as usize;
        assert_eq!(separation_method_count(config), expected_policies);
        let mut report = SeparationReport {
            candidate_observations: 1,
            active: vec![true],
            retained_observations: vec![0],
            dropped_observations: Vec::new(),
            drop_reasons: vec![None],
            singleton_count: 0,
            separated_count: 0,
            generations: 0,
            generation_hashes: vec![identity_hash],
            methods: Vec::new(),
        };
        if expected_policies == 0 {
            validate_separation_report(&report, config, &candidate_original_rows, 1, &[], 0)
                .expect("keepsingletons plus separation(none) has a zero-report envelope");
            continue;
        }
        report
            .methods
            .resize_with(expected_policies + 1, || SeparationMethodReport {
                generation: 0,
                method: SeparationMethod::Singletons,
                status: MethodStatus::NoSeparation,
                iterations: 1,
                dropped_observations: Vec::new(),
                singleton_drops: Vec::new(),
                certificate: None,
                exact_fe_certificate: None,
                elapsed_seconds: 0.0,
                workspace_observation_vectors: 0,
                simplex_tableau_cells: 0,
                note: "prepared singleton pruning independently certified a stable fixed point"
                    .into(),
            });
        assert!(
            validate_separation_report(&report, config, &candidate_original_rows, 1, &[], 0,)
                .is_err()
        );
    }

    let mut impossible_generation = SeparationReport {
        candidate_observations: 1,
        active: vec![true],
        retained_observations: vec![0],
        dropped_observations: Vec::new(),
        drop_reasons: vec![None],
        singleton_count: 0,
        separated_count: 0,
        generations: 1,
        generation_hashes: vec![identity_hash, identity_hash],
        methods: Vec::new(),
    };
    let disabled = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    assert!(validate_separation_report(
        &impossible_generation,
        disabled,
        &candidate_original_rows,
        1,
        &[],
        0,
    )
    .is_err());
    impossible_generation.generations = 0;
    assert!(validate_separation_report(
        &impossible_generation,
        disabled,
        &candidate_original_rows,
        1,
        &[],
        0,
    )
    .is_err());
}

#[test]
fn exact_fe_identities_bind_intercept_terms_and_direct_drop_anchors() {
    use crate::metadata::AbsorbTermId;
    use crate::ppml_separation::{ExactFeLevelIdentity, ExactFeSeparationCertificate};

    let fixed_effect = |declared_channel, term_id, kind| PpmlFixedEffectResult {
        declared_channel,
        term_id,
        source_ids: vec![term_id],
        kind,
        slope_source_id: (kind == PpmlFixedEffectChannelKind::Slope).then_some(term_id),
        levels: 3,
        categories: 3,
        redundant_parameters: 0,
        absorbed_rank: 2,
        redundancy_exact: true,
        nested: false,
        saved_channel: None,
        normalization_applied: true,
        normalization_certified: true,
        reconstruction_certified: true,
    };
    let fixed_effects = vec![
        fixed_effect(0, 1, PpmlFixedEffectChannelKind::Intercept),
        fixed_effect(1, 1, PpmlFixedEffectChannelKind::Slope),
        fixed_effect(2, 2, PpmlFixedEffectChannelKind::Intercept),
        fixed_effect(3, 3, PpmlFixedEffectChannelKind::Slope),
    ];
    let candidate_original_rows = vec![10_usize, 20, 30];
    let mut method = SeparationMethodReport {
        generation: 1,
        method: SeparationMethod::FixedEffects,
        status: MethodStatus::Certified,
        iterations: 1,
        dropped_observations: vec![0, 1],
        singleton_drops: Vec::new(),
        certificate: None,
        exact_fe_certificate: Some(ExactFeSeparationCertificate {
            checked_intercept_effects: 2,
            zero_only_levels: vec![
                ExactFeLevelIdentity {
                    effect: AbsorbTermId::new(1).expect("first intercept term"),
                    representative_original_row: 10,
                },
                ExactFeLevelIdentity {
                    effect: AbsorbTermId::new(2).expect("second intercept term"),
                    representative_original_row: 20,
                },
            ],
            passed: true,
        }),
        elapsed_seconds: 0.0,
        workspace_observation_vectors: 1,
        simplex_tableau_cells: 0,
        note: "prepared exact zero-only FE certificate passed".into(),
    };
    assert_eq!(
        validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects)
            .expect("stable exact-FE identities"),
        2
    );

    let certificate = method
        .exact_fe_certificate
        .as_mut()
        .expect("exact-FE certificate");
    certificate.zero_only_levels[1].representative_original_row = 30;
    assert!(
        validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects).is_err()
    );
    method
        .exact_fe_certificate
        .as_mut()
        .expect("exact-FE certificate")
        .zero_only_levels[1] = ExactFeLevelIdentity {
        effect: AbsorbTermId::new(3).expect("slope-only term"),
        representative_original_row: 20,
    };
    assert!(
        validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects).is_err()
    );
    let first = method
        .exact_fe_certificate
        .as_ref()
        .expect("exact-FE certificate")
        .zero_only_levels[0];
    method
        .exact_fe_certificate
        .as_mut()
        .expect("exact-FE certificate")
        .zero_only_levels[1] = first;
    assert!(
        validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects).is_err()
    );

    method.status = MethodStatus::NoSeparation;
    method.dropped_observations.clear();
    method.note = "prepared exact FE scan found no zero-only intercept level".into();
    method
        .exact_fe_certificate
        .as_mut()
        .expect("exact-FE certificate")
        .zero_only_levels
        .clear();
    assert_eq!(
        validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects)
            .expect("empty exact-FE set proves no separation"),
        0
    );
    method
        .exact_fe_certificate
        .as_mut()
        .expect("exact-FE certificate")
        .zero_only_levels
        .push(first);
    assert!(
        validate_separation_method(&method, &candidate_original_rows, 1, &fixed_effects).is_err()
    );
}

#[test]
fn simplex_full_positive_rank_proof_is_bound_to_the_final_finite_dimension() {
    use crate::ppml_separation::SeparationCertificateProof;

    let mut report = certified_composite_input().separation;
    report.methods.push(SeparationMethodReport {
        generation: 0,
        method: SeparationMethod::Simplex,
        status: MethodStatus::NoSeparation,
        iterations: 1,
        dropped_observations: Vec::new(),
        singleton_drops: Vec::new(),
        certificate: Some(SeparationCertificate {
            proof: SeparationCertificateProof::SimplexFullPositiveRank { columns: 1 },
            interior_zero_relative: 0.0,
            boundary_violation_relative: 0.0,
            span_residual_relative: 0.0,
            negative_margin_relative: 0.0,
            direction_scale: 0.0,
            positive_maximum: 0.0,
            passed: true,
        }),
        exact_fe_certificate: None,
        elapsed_seconds: 0.0,
        workspace_observation_vectors: 0,
        simplex_tableau_cells: 1,
        note: "finite regressors have full column rank on the positive-outcome sample".into(),
    });
    let candidate_original_rows = (0..report.candidate_observations).collect::<Vec<_>>();
    let simplex_only = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: false,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    validate_separation_report(&report, simplex_only, &candidate_original_rows, 1, &[], 0)
        .expect("full-positive-rank proof closes simplex exactly");

    let mut wrong_dimension = report.clone();
    wrong_dimension.methods[0]
        .certificate
        .as_mut()
        .expect("simplex certificate")
        .proof = SeparationCertificateProof::SimplexFullPositiveRank { columns: 2 };
    let error = validate_separation_report(
        &wrong_dimension,
        simplex_only,
        &candidate_original_rows,
        1,
        &[],
        0,
    )
    .expect_err("proof must bind the final finite dimension");
    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(error.return_code(), 3499);

    let mut failed = report;
    failed.methods[0]
        .certificate
        .as_mut()
        .expect("simplex certificate")
        .passed = false;
    let error =
        validate_separation_report(&failed, simplex_only, &candidate_original_rows, 1, &[], 0)
            .expect_err("failed exact proof is not publishable");
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert_eq!(error.return_code(), 430);
}

#[test]
fn unresolved_simplex_requires_a_certified_comprehensive_ir_closure() {
    use crate::ppml_separation::SeparationCertificateProof;

    let mut unresolved = certified_composite_input().separation;
    unresolved.methods.push(SeparationMethodReport {
        generation: 0,
        method: SeparationMethod::Simplex,
        status: MethodStatus::Uncertified,
        iterations: 1,
        dropped_observations: Vec::new(),
        singleton_drops: Vec::new(),
        certificate: None,
        exact_fe_certificate: None,
        elapsed_seconds: 0.0,
        workspace_observation_vectors: 0,
        simplex_tableau_cells: 1,
        note: "model simplex phase-I dual failed independent Farkas verification".into(),
    });
    let candidate_original_rows = (0..unresolved.candidate_observations).collect::<Vec<_>>();
    let simplex_only = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: false,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let error = validate_separation_report(
        &unresolved,
        simplex_only,
        &candidate_original_rows,
        1,
        &[],
        0,
    )
    .expect_err("terminal simplex uncertainty must fail closed");
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert_eq!(error.return_code(), 430);

    unresolved.methods.push(SeparationMethodReport {
        generation: 0,
        method: SeparationMethod::IteratedRectifier,
        status: MethodStatus::NoSeparation,
        iterations: 1,
        dropped_observations: Vec::new(),
        singleton_drops: Vec::new(),
        certificate: Some(SeparationCertificate {
            proof: SeparationCertificateProof::IteratedRectifierFixedPoint,
            interior_zero_relative: 0.0,
            boundary_violation_relative: 0.0,
            span_residual_relative: 0.0,
            negative_margin_relative: 0.0,
            direction_scale: 1.0,
            positive_maximum: 0.0,
            passed: true,
        }),
        exact_fe_certificate: None,
        elapsed_seconds: 0.0,
        workspace_observation_vectors: 8,
        simplex_tableau_cells: 0,
        note: "prepared iterated rectifier certified no separation".into(),
    });
    let simplex_ir = SeparationConfig {
        methods: SeparationMethods {
            iterated_rectifier: true,
            ..simplex_only.methods
        },
        ..simplex_only
    };
    validate_separation_report(&unresolved, simplex_ir, &candidate_original_rows, 1, &[], 0)
        .expect("same-generation comprehensive IR closes simplex uncertainty");
}

#[test]
fn ir_commit_does_not_retroactively_close_simplex_across_generations() {
    use crate::ppml_separation::SeparationCertificateProof;

    let candidate_original_rows = vec![0_usize, 1, 2, 3];
    let initial_active = vec![true; 4];
    let initial_reasons = vec![None; 4];
    let committed_active = vec![false, true, true, true];
    let committed_reasons = vec![
        Some(SeparationDropReason::IteratedRectifier),
        None,
        None,
        None,
    ];
    let mut report = SeparationReport {
        candidate_observations: 4,
        active: committed_active.clone(),
        retained_observations: vec![1, 2, 3],
        dropped_observations: vec![0],
        drop_reasons: committed_reasons.clone(),
        singleton_count: 0,
        separated_count: 1,
        generations: 1,
        generation_hashes: vec![
            candidate_ledger_hash_candidate_axis(
                &candidate_original_rows,
                &initial_active,
                &initial_reasons,
            )
            .expect("initial ledger hash"),
            candidate_ledger_hash_candidate_axis(
                &candidate_original_rows,
                &committed_active,
                &committed_reasons,
            )
            .expect("committed ledger hash"),
        ],
        methods: vec![
            SeparationMethodReport {
                generation: 0,
                method: SeparationMethod::Simplex,
                status: MethodStatus::Uncertified,
                iterations: 1,
                dropped_observations: Vec::new(),
                singleton_drops: Vec::new(),
                certificate: None,
                exact_fe_certificate: None,
                elapsed_seconds: 0.0,
                workspace_observation_vectors: 0,
                simplex_tableau_cells: 1,
                note: "model simplex phase-I dual failed independent Farkas verification".into(),
            },
            SeparationMethodReport {
                generation: 1,
                method: SeparationMethod::IteratedRectifier,
                status: MethodStatus::Certified,
                iterations: 1,
                dropped_observations: vec![0],
                singleton_drops: Vec::new(),
                certificate: Some(SeparationCertificate {
                    proof: SeparationCertificateProof::RecessionDirection,
                    interior_zero_relative: 0.0,
                    boundary_violation_relative: 0.0,
                    span_residual_relative: 0.0,
                    negative_margin_relative: 1.0,
                    direction_scale: 1.0,
                    positive_maximum: 0.0,
                    passed: true,
                }),
                exact_fe_certificate: None,
                elapsed_seconds: 0.0,
                workspace_observation_vectors: 8,
                simplex_tableau_cells: 0,
                note: "prepared iterated-rectifier candidate direction".into(),
            },
        ],
    };
    let simplex_ir = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: true,
            iterated_rectifier: true,
            mu: false,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let error =
        validate_separation_report(&report, simplex_ir, &candidate_original_rows, 1, &[], 0)
            .expect_err("an IR deletion cannot certify the restarted generation");
    assert_eq!(error.code, ErrorCode::NonConvergence);

    report.methods.extend([
        SeparationMethodReport {
            generation: 1,
            method: SeparationMethod::Simplex,
            status: MethodStatus::NoSeparation,
            iterations: 0,
            dropped_observations: Vec::new(),
            singleton_drops: Vec::new(),
            certificate: Some(SeparationCertificate {
                proof: SeparationCertificateProof::SimplexFullPositiveRank { columns: 1 },
                interior_zero_relative: 0.0,
                boundary_violation_relative: 0.0,
                span_residual_relative: 0.0,
                negative_margin_relative: 0.0,
                direction_scale: 0.0,
                positive_maximum: 0.0,
                passed: true,
            }),
            exact_fe_certificate: None,
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 0,
            simplex_tableau_cells: 1,
            note: "finite regressors have full column rank on the positive-outcome sample".into(),
        },
        SeparationMethodReport {
            generation: 1,
            method: SeparationMethod::IteratedRectifier,
            status: MethodStatus::NoSeparation,
            iterations: 1,
            dropped_observations: Vec::new(),
            singleton_drops: Vec::new(),
            certificate: Some(SeparationCertificate {
                proof: SeparationCertificateProof::IteratedRectifierFixedPoint,
                interior_zero_relative: 0.0,
                boundary_violation_relative: 0.0,
                span_residual_relative: 0.0,
                negative_margin_relative: 0.0,
                direction_scale: 0.0,
                positive_maximum: 0.0,
                passed: true,
            }),
            exact_fe_certificate: None,
            elapsed_seconds: 0.0,
            workspace_observation_vectors: 8,
            simplex_tableau_cells: 0,
            note: "prepared iterated rectifier certified no separation".into(),
        },
    ]);
    validate_separation_report(&report, simplex_ir, &candidate_original_rows, 1, &[], 0)
        .expect("the restarted generation has its own complete detector chain");

    let mut missing_commit = report;
    missing_commit.generation_hashes.pop();
    let error = validate_separation_report(
        &missing_commit,
        simplex_ir,
        &candidate_original_rows,
        1,
        &[],
        0,
    )
    .expect_err("IR closure requires the committed generation receipt");
    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(error.return_code(), 3499);
}

#[test]
fn zero_direction_is_only_valid_for_exact_zero_no_separation_certificate() {
    let certificate = SeparationCertificate {
        proof: crate::ppml_separation::SeparationCertificateProof::RecessionDirection,
        interior_zero_relative: 0.0,
        boundary_violation_relative: 0.0,
        span_residual_relative: 0.0,
        negative_margin_relative: 0.0,
        direction_scale: 0.0,
        positive_maximum: 0.0,
        passed: true,
    };
    validate_separation_certificate(&certificate, true)
        .expect("zero numerical direction is valid no-separation evidence");
    assert!(validate_separation_certificate(&certificate, false).is_err());

    let mut nonzero_diagnostic = certificate;
    nonzero_diagnostic.span_residual_relative = f64::EPSILON;
    assert!(validate_separation_certificate(&nonzero_diagnostic, true).is_err());
}

#[test]
fn mu_no_drop_grammar_is_proof_free_no_candidates_only() {
    use crate::ppml_separation::SeparationCertificateProof;

    let mut report = certified_composite_input().separation;
    report.methods.push(SeparationMethodReport {
        generation: 0,
        method: SeparationMethod::Mu,
        status: MethodStatus::NoSeparation,
        iterations: 1,
        dropped_observations: Vec::new(),
        singleton_drops: Vec::new(),
        certificate: Some(SeparationCertificate {
            proof: SeparationCertificateProof::RecessionDirection,
            interior_zero_relative: 0.0,
            boundary_violation_relative: 0.0,
            span_residual_relative: 0.0,
            negative_margin_relative: 0.0,
            direction_scale: 0.0,
            positive_maximum: 0.0,
            passed: true,
        }),
        exact_fe_certificate: None,
        elapsed_seconds: 0.0,
        workspace_observation_vectors: 8,
        simplex_tableau_cells: 0,
        note: "forged Mu no-separation claim".into(),
    });
    let candidate_original_rows = (0..report.candidate_observations).collect::<Vec<_>>();
    let mu_only = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: true,
        },
        drop_singletons: false,
        ..SeparationConfig::default()
    };
    let error = validate_separation_report(&report, mu_only, &candidate_original_rows, 1, &[], 0)
        .expect_err("Mu cannot certify absence of separation");
    assert_eq!(error.code, ErrorCode::NonConvergence);

    let terminal_mu = report.methods.last_mut().expect("Mu report");
    terminal_mu.status = MethodStatus::NoCandidates;
    terminal_mu.iterations = 0;
    terminal_mu.certificate = None;
    terminal_mu.note = "prepared mu found no small zero-outcome means".into();
    validate_separation_report(&report, mu_only, &candidate_original_rows, 1, &[], 0)
        .expect("proof-free Mu no-candidate result is the sole no-drop grammar");
}

#[test]
fn clipped_covariance_validation_certifies_the_published_matrix() {
    // Q is an independently constructed Householder reflection. The raw
    // spectrum has 22 positive and 43 negative eigenvalues, so its exact
    // PSD projection is Q diag(max(lambda, 0)) Q'.
    let dimension = 65;
    let tolerance = 1e-10;
    let vector: Vec<f64> = (0..dimension)
        .map(|i| (((i + 1) * 137 + 11 * 53) % 251) as f64 - 125.0)
        .collect();
    let squared_norm: f64 = vector.iter().map(|value| value * value).sum();
    let orthogonal: Vec<f64> = (0..dimension * dimension)
        .map(|i| {
            f64::from(u8::from(i / dimension == i % dimension))
                - 2.0 * vector[i / dimension] * vector[i % dimension] / squared_norm
        })
        .collect();
    let eigenvalues: Vec<f64> = (0..dimension)
        .map(|j| {
            if j % 3 == 0 {
                1.0 + j as f64
            } else {
                -(1.0 + j as f64) / 7.0
            }
        })
        .collect();
    let mut raw = vec![0.0; dimension * dimension];
    let mut expected = vec![0.0; dimension * dimension];
    for row in 0..dimension {
        for column in row..dimension {
            let mut value = 0.0;
            let mut clipped = 0.0;
            for eigen in 0..dimension {
                value += orthogonal[row * dimension + eigen]
                    * eigenvalues[eigen]
                    * orthogonal[column * dimension + eigen];
                clipped += orthogonal[row * dimension + eigen]
                    * eigenvalues[eigen].max(0.0)
                    * orthogonal[column * dimension + eigen];
            }
            raw[row * dimension + column] = value;
            raw[column * dimension + row] = value;
            expected[row * dimension + column] = clipped;
            expected[column * dimension + row] = clipped;
        }
    }
    let (published, report) = apply_ppml_psd_policy(&raw, dimension, 2, tolerance).unwrap();
    assert_eq!(report.final_rank, 22);
    assert_eq!(report.clipped_eigenvalues, 43);
    assert!(matrix_relative_error(&published, &expected).unwrap() < tolerance);
    let coefficients = PpmlCoefficientOutput::new(
        vec![0.0; dimension],
        vec![PpmlCoefficientStatus::Identified; dimension],
        (0..dimension).map(Some).collect(),
        (0..dimension).collect(),
    )
    .unwrap();
    let mut inference = certified_composite_input().inference;
    inference.vce.covariance = published.clone();
    inference.vce.standard_errors = (0..dimension)
        .map(|index| published[index * dimension + index].sqrt())
        .collect();
    inference.vce.cluster_dimension_groups = vec![97, 3];
    inference.vce.psd = report;
    validate_covariance_and_certificates(&coefficients, &inference, tolerance)
        .expect("the certificate must describe the actual published covariance");
    assert_eq!(inference.vce.covariance, published);
    for mutation in 0..8 {
        inference.vce.psd = report;
        match mutation {
            0 => inference.vce.psd.final_rank += 1,
            1 => inference.vce.psd.clipped_eigenvalues = 0,
            2 => inference.vce.psd.adjusted = false,
            3 => inference.vce.psd.raw_minimum_eigenvalue = 0.0,
            4 => inference.vce.psd.final_minimum_eigenvalue = 1.0,
            5 => inference.vce.psd.orthogonality_relative_error = 0.0,
            6 => inference.vce.psd.eigensystem_reconstruction_relative_error = 0.0,
            _ => inference.vce.psd.passed = false,
        }
        assert_eq!(
            validate_covariance_and_certificates(&coefficients, &inference, tolerance)
                .expect_err("an inconsistent report must still be rejected")
                .code,
            ErrorCode::NonConvergence,
        );
    }
    inference.vce.psd = report;
    inference.vce.covariance[1] = 1000.0;
    inference.vce.covariance[dimension] = 1000.0;
    assert_eq!(
        validate_covariance_and_certificates(&coefficients, &inference, tolerance)
            .expect_err("a materially indefinite matrix cannot reuse a valid report")
            .code,
        ErrorCode::NonConvergence,
    );
}

#[test]
fn identified_zero_variance_uses_zero_se_and_missing_z() {
    let mut input = certified_composite_input();
    input.inference.vce.covariance[0] = 0.0;
    input.inference.vce.standard_errors[0] = 0.0;
    input.inference.vce.z_statistics[0] = f64::NAN;
    input.inference.vce.psd = apply_ppml_psd_policy(&[0.0], 1, 0, 1e-10)
        .expect("zero covariance PSD certificate")
        .1;
    CertifiedPpmlResult::new(input).expect("PSD-singular identified inference");

    let mut mismatch = certified_composite_input();
    mismatch.inference.vce.covariance[0] = 0.0;
    mismatch.inference.vce.standard_errors[0] = 0.0;
    mismatch.inference.vce.z_statistics[0] = 0.0;
    mismatch.inference.vce.psd = apply_ppml_psd_policy(&[0.0], 1, 0, 1e-10)
        .expect("zero covariance PSD certificate")
        .1;
    assert_eq!(
        CertifiedPpmlResult::new(mismatch)
            .expect_err("zero SE must not report a finite z")
            .code,
        ErrorCode::Internal
    );
}

#[test]
fn normalized_structural_constant_must_be_exactly_zero() {
    let error = PpmlCoefficientOutput::new(
        vec![1.0],
        vec![PpmlCoefficientStatus::NormalizedStructuralConstant],
        vec![Some(0)],
        Vec::new(),
    )
    .expect_err("nonzero normalized constant must be rejected");
    assert_eq!(error.code, ErrorCode::Internal);
}

#[test]
fn fixed_effect_rows_require_explicit_channel_identity_and_exact_rank_sum() {
    let output = exact_state(true);
    let effects = vec![
        PpmlFixedEffectResult {
            declared_channel: 0,
            term_id: 1,
            source_ids: vec![1],
            kind: PpmlFixedEffectChannelKind::Intercept,
            slope_source_id: None,
            levels: 2,
            categories: 2,
            redundant_parameters: 1,
            absorbed_rank: 1,
            redundancy_exact: true,
            nested: false,
            saved_channel: Some(0),
            normalization_applied: true,
            normalization_certified: true,
            reconstruction_certified: true,
        },
        PpmlFixedEffectResult {
            declared_channel: 1,
            term_id: 1,
            source_ids: vec![1],
            kind: PpmlFixedEffectChannelKind::Slope,
            slope_source_id: Some(2),
            levels: 2,
            categories: 2,
            redundant_parameters: 0,
            absorbed_rank: 2,
            redundancy_exact: false,
            nested: false,
            saved_channel: Some(1),
            normalization_applied: true,
            normalization_certified: true,
            reconstruction_certified: true,
        },
    ];
    let coefficients = PpmlCoefficientOutput::new(
        vec![0.0],
        vec![PpmlCoefficientStatus::NormalizedStructuralConstant],
        vec![Some(0)],
        Vec::new(),
    )
    .expect("normalized constant");
    validate_fixed_effect_metadata(&effects, &output, 3, &coefficients).expect("valid FE rows");

    let mut missing_slope_identity = effects.clone();
    missing_slope_identity[1].slope_source_id = None;
    assert_eq!(
        validate_fixed_effect_metadata(&missing_slope_identity, &output, 3, &coefficients)
            .expect_err("slope identity")
            .code,
        ErrorCode::Internal
    );
    assert_eq!(
        validate_fixed_effect_metadata(&effects, &output, 2, &coefficients)
            .expect_err("rank mismatch")
            .code,
        ErrorCode::Internal
    );
    let mut inconsistent_levels = effects.clone();
    inconsistent_levels[0].levels = 3;
    assert_eq!(
        validate_fixed_effect_metadata(&inconsistent_levels, &output, 3, &coefficients)
            .expect_err("level/category identity")
            .code,
        ErrorCode::Internal
    );
    let mut unapplied_saved_normalization = effects.clone();
    unapplied_saved_normalization[0].normalization_applied = false;
    assert_eq!(
        validate_fixed_effect_metadata(&unapplied_saved_normalization, &output, 3, &coefficients,)
            .expect_err("saved normalization evidence")
            .code,
        ErrorCode::Internal
    );
}
