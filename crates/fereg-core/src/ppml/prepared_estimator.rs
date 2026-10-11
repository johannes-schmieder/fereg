//! One owned, certified production boundary for prepared PPML.
//!
//! This module is deliberately the only composition layer that is allowed to
//! join numeric preparation, categorical topology, separation, the reusable
//! weighted-linear session, PPML inference, and publication metadata.  It
//! never calls the legacy dense PPML fitter or its legacy covariance path.

use super::constant_wald::constant_wald_columns;
use crate::memory::{fallible_copy, fallible_filled};
use std::time::Duration;

use crate::data::{NumericColumnView, NumericSourceUse, WeightInput, WeightKind};
use crate::error::{allocation, invalid_argument, ErrorCode, FeregError, Result};
use crate::fe::FeSolverKind;
use crate::ols::{
    LinearSessionPhaseAllocatorProbe, LoadedNumericColumn, LoadedOlsInput, OlsClusterTermV1,
    OlsRequestV1, PreparedOlsColumnKind, PreparedOlsFitConfig, PreparedOlsFitStatus,
    PreparedOlsLoadHint,
};
use crate::options::MemoryMode;
use crate::ppml_separation::{
    separation_generation_capacity, separation_method_count, separation_workspace_requirements,
    SeparationConfig, SeparationMethod,
};

use super::memory::{begin_phase_allocation, end_phase_allocation, plan_ppml_memory_with_probe};
use super::preparation_trace::PreparationTrace;
use super::prepared::{
    classify_ppml_numeric_data, select_ppml_numeric_data, PpmlNumericClassification,
    PpmlNumericPreparationInput, PreparedPpmlAdjustmentKind,
};
use super::prepared_irls::{
    fit_generation_ppml_irls_interruptible, PreparedPpmlIrlsConfig, PreparedPpmlIrlsFit,
    PreparedPpmlIrlsPurpose, PreparedPpmlIrlsRun,
};
use super::prepared_model::{
    PreparedPpmlModel, PreparedPpmlModelGeneration, PreparedPpmlModelOwner,
};
use super::prepared_separation::{
    fit_with_prepared_ppml_separation, PreparedPpmlSeparationExecution,
};
use super::result::{
    prepare_ppml_fixed_effect_report, recover_ppml_fixed_effects, PreparedPpmlFixedEffectReport,
};
use super::vce::compute_ppml_inference_interruptible_with_threads;
use super::{
    CertifiedPpmlResult, CertifiedPpmlResultInput, PpmlAllocatorSnapshot,
    PpmlAllocatorSnapshotProbe, PpmlClusterDimensionResult, PpmlCoefficientOutput,
    PpmlCoefficientStatus, PpmlConfig, PpmlConvergenceReport, PpmlFinalSample, PpmlMemoryConfig,
    PpmlMemoryDimensions, PpmlMemoryRoute, PpmlOutputState, PpmlPhase, PpmlPhaseAllocatorProbe,
    PpmlPhaseClock, PpmlResidualKind, PpmlSolverFamily, PpmlSolverPlan, PpmlVceRequest,
};

#[cfg(test)]
use super::plan_ppml_memory;
#[cfg(test)]
use super::prepared::prepare_ppml_numeric_data;

/// Publication-sized products requested by the embedding layer.
///
/// Mandatory scientific state is borrowed from the strict fit for composite
/// certification. Only products named here cross the result boundary; the
/// single certification-only mean copy is released immediately afterward.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreparedPpmlOutputRequest {
    pub residual: Option<PpmlResidualKind>,
    pub xb: bool,
    pub absorbed: bool,
    pub eta: bool,
    pub mean: bool,
    /// Exact count of residual/xb/absorbed/eta/mu and requested saved-FE
    /// channels. This redundant check binds preflight to the decoded
    /// transaction instead of trusting a caller-supplied scalar alone.
    pub requested_observation_columns: usize,
    pub publication_matrix_cells: usize,
}

impl PreparedPpmlOutputRequest {
    fn non_saved_columns(self) -> usize {
        usize::from(self.residual.is_some())
            + usize::from(self.xb)
            + usize::from(self.absorbed)
            + usize::from(self.eta)
            + usize::from(self.mean)
    }

    fn validate(self, saved_channels: usize) -> Result<()> {
        let expected = self
            .non_saved_columns()
            .checked_add(saved_channels)
            .ok_or_else(|| allocation("PPML requested output-column count overflows"))?;
        if self.requested_observation_columns != expected {
            return Err(invalid_argument(
                "PPML requested output-column count disagrees with its exact product mask",
            ));
        }
        Ok(())
    }
}

/// Complete estimator policy.  The OLS request supplies only shared model,
/// categorical, binding, and singleton metadata; all likelihood, covariance,
/// memory, and separation choices remain PPML-owned.
#[allow(unpredictable_function_pointer_comparisons)]
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedPpmlRequestV1 {
    pub bias: Option<super::PpmlBiasRequest>,
    pub ols: OlsRequestV1,
    pub clusters: Vec<OlsClusterTermV1>,
    pub vce: PpmlVceRequest,
    pub ppml: PpmlConfig,
    pub linear: PreparedOlsFitConfig,
    pub memory: PpmlMemoryConfig,
    /// Optional qualification observer at the memory-planner decision
    /// boundary, before any fit workspace is allocated.
    pub memory_preflight_probe: Option<super::PpmlMemoryPreflightProbe>,
    /// Optional allocation-free counter probe for warmed-iteration
    /// qualification. Normal fits leave this `None`.
    pub allocation_snapshot_probe: Option<PpmlAllocatorSnapshotProbe>,
    /// Qualification-only phase peak observer. Normal fits leave this `None`.
    pub phase_allocation_probe: Option<PpmlPhaseAllocatorProbe>,
    /// Qualification-only neutral observer for projection and finite-solve
    /// phases inside the shared linear session. Normal fits leave this `None`.
    pub linear_phase_allocation_probe: Option<LinearSessionPhaseAllocatorProbe>,
    /// Action-lifetime allocator probe. Unlike `allocation_snapshot_probe`,
    /// this never enables the warmed-iteration qualification window.
    pub action_allocation_snapshot_probe: Option<PpmlAllocatorSnapshotProbe>,
    /// Snapshot captured by the embedding layer before parse/load work.
    pub action_allocation_start: Option<PpmlAllocatorSnapshot>,
    /// Prior action's terminal allocation baseline, when observed.
    pub action_allocation_previous_baseline_bytes: Option<u64>,
    pub separation: SeparationConfig,
    pub output: PreparedPpmlOutputRequest,
    /// Public PPML fixed-effect solver family. `Auto` remains a distinct
    /// request identity even while its selected family is conservatively MAP.
    pub solver: PpmlSolverFamily,
    /// Hidden, action-local qualification hook.  It reaches the ordinary
    /// fallible FE-recovery reservation; it never alters process-global
    /// allocator state and therefore cannot leak into a later action.
    #[doc(hidden)]
    pub inject_fe_recovery_allocation_failure: bool,
}

/// Owned raw inputs for PPML preparation.
///
/// Exposure remains on its user scale here and is logged exactly once by the
/// numeric preparation boundary.  `adjustment` must be absent for `None` and
/// present for `Offset` or `Exposure`.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPpmlInput {
    /// Does not participate in ordinary sample selection.
    pub bias_units: Option<LoadedNumericColumn>,
    pub ols: LoadedOlsInput,
    pub adjustment: Option<LoadedNumericColumn>,
    pub adjustment_kind: PreparedPpmlAdjustmentKind,
}

/// One call's raw input and optional transient categorical preparation work.
/// Existing raw-input callers need no capsule or additional allocation.
/// Embeddings exclude the capsule's exact owned capacity from any permanent
/// embedding charge; core admits it only within the existing preparation bank.
pub struct PpmlInput {
    loaded: LoadedPpmlInput,
    load_hint: Option<PreparedOlsLoadHint>,
}

impl From<LoadedPpmlInput> for PpmlInput {
    fn from(loaded: LoadedPpmlInput) -> Self {
        (loaded, None).into()
    }
}

impl From<(LoadedPpmlInput, Option<PreparedOlsLoadHint>)> for PpmlInput {
    fn from((loaded, load_hint): (LoadedPpmlInput, Option<PreparedOlsLoadHint>)) -> Self {
        Self { loaded, load_hint }
    }
}

/// Caller-thread safe points exposed by the interruptible PPML entry.
///
/// The callback is deliberately accepted without `Send` or `Sync`: embeddings
/// such as Stata may poll only on the thread that entered the native plugin.
/// Long parallel kernels therefore poll immediately before or after their
/// caller-thread boundary, never from a Rayon worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlInterruptPoint {
    BiasCorrection,
    Preparation,
    SeparationGeneration,
    SeparationFixedEffects,
    SeparationSimplex,
    SeparationSimplexPivot,
    SeparationIteratedRectifier,
    SeparationIrIteration,
    SeparationMu,
    IrlsIteration,
    IrlsLineSearchTrial,
    StrictFinalSolve,
    Inference,
    InferenceSubset,
    FeRecovery,
    Publication,
}

/// Fit and certify PPML through the prepared production architecture.
///
/// Every intermediate is owned locally.  The function publishes nothing
/// until `CertifiedPpmlResult::new` has independently revalidated the complete
/// final-generation composition, so every error is atomic to callers.
pub fn fit_prepared_ppml(
    request: PreparedPpmlRequestV1,
    loaded: LoadedPpmlInput,
) -> Result<CertifiedPpmlResult> {
    fit_prepared_ppml_interruptible(request, loaded, &mut |_| Ok(()))
}

/// Fit PPML while invoking a caller-owned interruption probe at safe serial
/// boundaries. The probe is never moved into a worker closure.
pub fn fit_prepared_ppml_interruptible(
    mut request: PreparedPpmlRequestV1,
    input: impl Into<PpmlInput>,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<CertifiedPpmlResult> {
    let PpmlInput {
        loaded,
        mut load_hint,
    } = input.into();
    validate_request(&request, &loaded)?;
    let solver_plan = PpmlSolverPlan::new(request.solver);
    request.memory.require_fast_route = matches!(
        solver_plan.selected,
        PpmlSolverFamily::Lsmr | PpmlSolverFamily::Cmg
    );
    interrupt(PpmlInterruptPoint::Preparation)?;
    // Singleton policy belongs to the model request.  Derive the detector
    // switch here so two caller-controlled booleans cannot silently disagree.
    request.separation.drop_singletons = !request.ols.keep_singletons;
    let requested_saved_channel_count = requested_saved_channel_count(&request.ols)?;
    request.output.validate(requested_saved_channel_count)?;
    // Resolve the PPML absorption policy exactly once before handing it to
    // either separation or IRLS. In particular, leaving PPML at `Auto` while
    // resolving only the linear copy to `Map` creates two nominally valid but
    // unequal numerical policies at the production boundary.
    if request.inject_fe_recovery_allocation_failure && requested_saved_channel_count == 0 {
        return Err(invalid_argument(
            "PPML FE-recovery allocation injection requires a requested saved fixed effect",
        ));
    }
    // Saved effects are recovered once from the accepted canonical Hessian;
    // the projector that computed it is not itself scientific authority.
    // Retaining FE coefficients throughout IRLS would disable the borrowed
    // categorical MAP fast lane for every preceding frame.
    request.ppml.absorb.retain_coefficients = false;
    request.ppml.absorb.solver = FeSolverKind::Map;
    request.linear.fixed_effect_collinearity_tolerance =
        request.ppml.fixed_effect_collinearity_tolerance;
    request.linear.absorb = request.ppml.absorb;

    // Preparation timing includes the allocation-free classification and
    // fit-lifetime route decision as well as the later materialization. The
    // allocator phase begins only after admission, so a rejected route still
    // proves that no fit workspace was opened.
    let preparation_clock = PpmlPhaseClock::start();
    let mut preparation_trace = PreparationTrace::new();

    // Build only borrowed column views and model-sized role metadata, then
    // classify the numeric sample without constructing any observation-sized
    // compact column. Route selection is frozen at this boundary (SPEC §1).
    let numeric_input = borrowed_numeric_input(&request, &loaded)?;
    let numeric_view = numeric_input.as_input();
    let views_seconds = preparation_trace.lap();
    let classified = classify_ppml_numeric_data(&numeric_view)?;
    let numeric_classification = classified.classification();
    let classification_seconds = preparation_trace.lap();
    let mut preflight_dimensions = conservative_preflight_dimensions_from_classification(
        &request,
        numeric_classification,
        numeric_input.numeric_sources.len(),
        numeric_input.offset.is_some() || numeric_input.exposure.is_some(),
        matches!(
            numeric_input.weights,
            WeightInput::Provided {
                kind: WeightKind::Probability,
                ..
            }
        ),
        requested_saved_channel_count,
    )?;
    preflight_dimensions.metadata.input_owner_bytes = raw_input_owner_bytes(&request, &loaded);
    load_hint = admit_load_hint(
        load_hint,
        numeric_classification,
        preflight_dimensions.metadata,
    );
    // This scalar refinement reads raw outcome values only after the row
    // classifier has resolved domain/error precedence. Excluded nonpositive
    // rows can only retain the wider envelope. No sample state is allocated.
    let mut all_positive = true;
    for chunk in loaded.ols.outcome.values.chunks(65_536) {
        interrupt(PpmlInterruptPoint::Preparation)?;
        if !chunk.iter().all(|value| *value > 0.0) {
            all_positive = false;
            break;
        }
    }
    preflight_dimensions.separation_generation_capacity = separation_generation_capacity(
        numeric_classification.n_active,
        all_positive,
        request.separation.drop_singletons,
    );
    let shape_seconds = preparation_trace.lap();
    refine_preflight_fe_bounds(
        &request.ols,
        &loaded.ols,
        &mut preflight_dimensions,
        interrupt,
    )?;
    let fe_bounds_seconds = preparation_trace.lap();
    if let Some(bias) = request.bias {
        preflight_dimensions.metadata.bias_workspace_bytes = super::bias::memory_bound(
            preflight_dimensions.observations,
            preflight_dimensions.fe_total_levels,
            preflight_dimensions.finite_columns,
            bias.save_effects,
        )?;
    }
    let preliminary_plan = plan_ppml_memory_with_probe(
        preflight_dimensions,
        request.memory,
        request.memory_preflight_probe,
    )?;
    let admission_seconds = preparation_trace.lap();

    begin_phase_allocation(request.phase_allocation_probe, PpmlPhase::Preparation);
    let numeric = select_ppml_numeric_data(classified)?;
    let selection_seconds = preparation_trace.lap();
    // Selection is bound to the classified input. Until topology exists,
    // retain the same proven FE envelope rather than widening it back to N.
    let prepared_dimensions = preflight_dimensions;
    drop(numeric_input);
    // The exact materialized shape may only bind within the earlier envelope;
    // it cannot silently reselect a route after canonical allocation.
    let preliminary_plan = preliminary_plan.with_final_dimensions(prepared_dimensions)?;
    let mut resolved_linear = request.linear;
    resolved_linear.execution.mode = match preliminary_plan.selected {
        PpmlMemoryRoute::Fast => crate::ols::PreparedOlsExecutionMode::Fast,
        PpmlMemoryRoute::Low => crate::ols::PreparedOlsExecutionMode::Low,
    };
    resolved_linear.execution.poolsize = request.memory.poolsize;
    let separation_execution = PreparedPpmlSeparationExecution {
        preliminary_plan,
        linear: resolved_linear,
    };
    let requested_saved_channels = requested_saved_channels(&request.ols)?;
    if requested_saved_channels
        .iter()
        .filter(|saved| **saved)
        .count()
        != requested_saved_channel_count
    {
        return Err(FeregError::new(
            ErrorCode::Internal,
            "PPML saved-channel materialization disagrees with its preflight count",
        ));
    }
    let omission_statuses = original_omission_statuses(&request.ols)?;
    let owner = PreparedPpmlModelOwner::from_selection(
        request.ols,
        request.clusters,
        loaded.ols,
        numeric,
        loaded.adjustment,
    )?
    .with_load_hint(load_hint)
    .with_generation_memory_bound(
        preliminary_plan
            .selected_inventory()
            .prepared_generation_bytes,
    );
    let owner_seconds = preparation_trace.lap();
    let preparation_elapsed = preparation_clock.elapsed();
    let preparation_peak =
        end_phase_allocation(request.phase_allocation_probe, PpmlPhase::Preparation);
    if preparation_trace.enabled() {
        eprintln!(
            "FEREG_PHASE|ppml_prepare_input|views={views_seconds:.9}|classification={classification_seconds:.9}|shape={shape_seconds:.9}|fe_bounds={fe_bounds_seconds:.9}|admission={admission_seconds:.9}|selection={selection_seconds:.9}|owner={owner_seconds:.9}|total={:.9}|rows={}|active_rows={}|team_workers={}",
            preparation_elapsed.as_secs_f64(),
            numeric_classification.n_original,
            numeric_classification.n_active,
            PreparationTrace::team_workers(),
        );
    }
    let ppml = request.ppml;
    let separation = request.separation;
    let output_request = request.output;
    let memory_config = request.memory;
    let allocation_snapshot_probe = request.allocation_snapshot_probe;
    let allocation_audit_enabled = allocation_snapshot_probe.is_some();
    let phase_allocation_probe = request.phase_allocation_probe;
    let linear_phase_allocation_probe = request.linear_phase_allocation_probe;
    let saved_channel_count = requested_saved_channels
        .iter()
        .filter(|requested| **requested)
        .count();
    let (separation_outcome, mut fit) = fit_with_prepared_ppml_separation(
        owner,
        ppml,
        separation,
        phase_allocation_probe,
        separation_execution,
        interrupt,
        |generation, preliminary_plan, resolved_linear, interrupt| {
            let cluster_codes = cluster_code_views(generation)?;
            let dimensions = final_memory_dimensions(
                generation,
                &cluster_codes,
                separation,
                output_request,
                saved_channel_count,
                resolved_linear,
                allocation_audit_enabled,
                preliminary_plan.preflight_dimensions.metadata,
            )?;
            let final_plan = (*preliminary_plan).with_final_dimensions(dimensions)?;
            match fit_generation_ppml_irls_interruptible(
                generation,
                PreparedPpmlIrlsConfig {
                    ppml,
                    linear: resolved_linear,
                    memory: memory_config,
                    // The private exploratory trajectory is charged to the
                    // outer Mu phase but must not mint or perturb the one
                    // publication-fit steady-state allocation certificate.
                    allocation_snapshot_probe: None,
                    phase_allocation_probe,
                    linear_phase_allocation_probe,
                    solver_plan,
                },
                final_plan,
                None,
                PreparedPpmlIrlsPurpose::MuExploration {
                    mu_tolerance: separation.mu_tolerance,
                },
                None,
                interrupt,
            )? {
                PreparedPpmlIrlsRun::MuExploration(exploration) => Ok(*exploration),
                _ => Err(invalid_argument(
                    "PPML exploration returned publication products",
                )),
            }
        },
        |generation, preliminary_plan, resolved_linear, interrupt| {
            let cluster_codes = cluster_code_views(generation)?;
            let dimensions = final_memory_dimensions(
                generation,
                &cluster_codes,
                separation,
                output_request,
                saved_channel_count,
                resolved_linear,
                allocation_audit_enabled,
                preliminary_plan.preflight_dimensions.metadata,
            )?;
            let final_plan = (*preliminary_plan).with_final_dimensions(dimensions)?;
            let mut bias_units = Vec::new();
            if let Some(units) = &loaded.bias_units {
                bias_units
                    .try_reserve_exact(generation.original_rows().len())
                    .map_err(|_| allocation("unable to compact bias units"))?;
                for &row in generation.original_rows() {
                    if units.missing[row] || !units.values[row].is_finite() {
                        return Err(invalid_argument(
                            "bias unit is missing on a retained estimation row",
                        ));
                    }
                    bias_units.push(units.values[row]);
                }
            }
            match fit_generation_ppml_irls_interruptible(
                generation,
                PreparedPpmlIrlsConfig {
                    ppml,
                    linear: resolved_linear,
                    memory: memory_config,
                    allocation_snapshot_probe,
                    phase_allocation_probe,
                    linear_phase_allocation_probe,
                    solver_plan,
                },
                final_plan,
                None,
                PreparedPpmlIrlsPurpose::Publication,
                request.bias.map(|b| (b, bias_units.as_slice())),
                interrupt,
            )? {
                PreparedPpmlIrlsRun::Publication(fit) => Ok(*fit),
                _ => Err(invalid_argument(
                    "PPML publication returned exploratory products",
                )),
            }
        },
    )?;
    fit.validate_generation(&separation_outcome.prepared)?;
    if request.bias.is_some() && separation_outcome.separation.separated_count > 0 {
        return Err(invalid_argument(
            "bias correction is unavailable after outcome-dependent separation deletions",
        ));
    }
    let final_drop_reasons = separation_outcome.final_drop_reasons()?;
    let prepared = &separation_outcome.prepared;
    let fixed_effect_report =
        prepare_ppml_fixed_effect_report(prepared.prepared(), &requested_saved_channels)?;
    let cluster_codes = cluster_code_views(prepared)?;

    let coefficients = map_coefficients(prepared.prepared(), &fit, &omission_statuses)?;
    let (wald_columns, wald_reference) = wald_columns(prepared.prepared(), &coefficients, &fit)?;
    let reported_finite_rank = coefficients
        .kept_full_slots()
        .len()
        .checked_add(
            coefficients
                .statuses()
                .iter()
                .filter(|status| **status == PpmlCoefficientStatus::NormalizedStructuralConstant)
                .count(),
        )
        .ok_or_else(|| allocation("PPML reported finite rank overflows"))?;
    begin_phase_allocation(request.phase_allocation_probe, PpmlPhase::Inference);
    let inference_clock = PpmlPhaseClock::start();
    interrupt(PpmlInterruptPoint::Inference)?;
    let inference = {
        let mut poll_subset = || interrupt(PpmlInterruptPoint::InferenceSubset);
        compute_ppml_inference_interruptible_with_threads(
            super::PpmlInferenceInput {
                coefficients: coefficients.values(),
                full_dimension: coefficients.values().len(),
                within_design: fit.linear().finite_within_design(),
                reported_finite_rank,
                score_residuals: fit.score_residuals(),
                prior_weights: prepared.prior_weights(),
                weight_report: prepared.weight_report(),
                identified_bread: fit.linear().identified_bread(),
                kept_full_slots: coefficients.kept_full_slots(),
                absorbed_rank: fixed_effect_report.absorbed_rank,
                connected_components: fixed_effect_report.connected_components,
                absorbed_rank_is_approximate: fixed_effect_report.is_approximate,
                clusters: &cluster_codes,
                requested_vce: request.vce,
                wald_columns: &wald_columns,
                wald_reference: wald_reference.as_ref(),
                certificate_tolerance: request.ppml.tolerance,
            },
            resolved_linear.execution.threads,
            &mut poll_subset,
        )?
    };
    interrupt(PpmlInterruptPoint::Inference)?;
    let inference_elapsed = inference_clock.elapsed();
    let inference_peak = end_phase_allocation(request.phase_allocation_probe, PpmlPhase::Inference);

    begin_phase_allocation(request.phase_allocation_probe, PpmlPhase::FeRecovery);
    let recovery_clock = PpmlPhaseClock::start();
    interrupt(PpmlInterruptPoint::FeRecovery)?;
    let (fixed_effects, saved_effects) = recover_saved_effects(
        prepared.prepared(),
        &fit,
        prepared.prior_weights(),
        fixed_effect_report,
        &requested_saved_channels,
        request.linear,
        request.ppml.tolerance,
        request.inject_fe_recovery_allocation_failure,
    )?;
    interrupt(PpmlInterruptPoint::FeRecovery)?;
    let recovery_elapsed = recovery_clock.elapsed();
    let recovery_peak = end_phase_allocation(request.phase_allocation_probe, PpmlPhase::FeRecovery);

    begin_phase_allocation(request.phase_allocation_probe, PpmlPhase::Publication);
    let publication_clock = PpmlPhaseClock::start();
    interrupt(PpmlInterruptPoint::Publication)?;
    let final_rows = std::sync::Arc::new(fallible_copy(
        prepared.original_rows(),
        "unable to copy PPML final-sample row ledger",
    )?);
    let sample = PpmlFinalSample::new(
        prepared.original_observations(),
        prepared.requested_candidate_observations(),
        separation_outcome.candidate_original_rows.clone(),
        final_rows.clone(),
        final_drop_reasons,
        fallible_copy(
            prepared.outcome(),
            "unable to copy PPML final-sample outcome",
        )?,
        fallible_copy(
            prepared.prior_weights(),
            "unable to copy PPML final-sample weights",
        )?,
    )?;
    let output = PpmlOutputState::new_requested(
        prepared.original_observations(),
        final_rows,
        prepared.offset(),
        fit.linear().finite_predictor(),
        fit.linear().absorbed_predictor(),
        fit.eta(),
        fit.mean(),
        prepared.outcome(),
        saved_effects,
        output_request,
        request.ppml.tolerance,
    )?;
    interrupt(PpmlInterruptPoint::Publication)?;
    let convergence = convergence_report(&fit)?;
    let clusters = cluster_metadata(prepared)?;
    let mut memory = fit.diagnostics().memory.clone();
    memory.record_phase(
        PpmlPhase::Preparation,
        preparation_elapsed + separation_outcome.initial_preparation_elapsed,
    );
    if let Some(bytes) = preparation_peak
        .into_iter()
        .chain(separation_outcome.initial_preparation_peak)
        .max()
    {
        memory.observe_phase_peak(PpmlPhase::Preparation, bytes);
    }
    record_separation_timings(&mut memory, &separation_outcome.separation)?;
    for phase in [
        PpmlPhase::SeparationFe,
        PpmlPhase::SeparationSimplex,
        PpmlPhase::SeparationIr,
        PpmlPhase::SeparationMu,
    ] {
        if let Some(bytes) = separation_outcome.phase_allocation_peak(phase) {
            memory.observe_phase_peak(phase, bytes);
        }
    }
    memory.record_phase(PpmlPhase::Inference, inference_elapsed);
    if let Some(bytes) = inference_peak {
        memory.observe_phase_peak(PpmlPhase::Inference, bytes);
    }
    memory.record_phase(PpmlPhase::FeRecovery, recovery_elapsed);
    if let Some(bytes) = recovery_peak {
        memory.observe_phase_peak(PpmlPhase::FeRecovery, bytes);
    }
    let result_input = CertifiedPpmlResultInput {
        bias: fit.take_bias(),
        coefficients,
        sample,
        weights: prepared.weight_report(),
        adjustment: prepared.adjustment_report(),
        objective: fit.objective(),
        inference,
        output,
        convergence,
        fixed_effects,
        clusters,
        separation: separation_outcome.separation,
        separation_config: separation,
        separation_seal: separation_outcome.certification_seal,
        memory,
        solver: fit.diagnostics().solver,
        certificate_tolerance: request.ppml.tolerance,
    };
    // Every certification input now owns its data. Release the final Hessian
    // products and prepared/raw generation before independent PSD, Wald and
    // separation replay allocate their scratch; no solver state is needed.
    drop(cluster_codes);
    drop(fit);
    drop(separation_outcome.prepared);
    drop(separation_outcome.original_axis_reasons);
    drop(separation_outcome.candidate_original_rows);
    drop(wald_columns);
    drop(wald_reference);
    drop(omission_statuses);
    drop(requested_saved_channels);
    drop(loaded.bias_units);
    let result = CertifiedPpmlResult::new(result_input)?.retain_requested_outputs();
    interrupt(PpmlInterruptPoint::Publication)?;
    result.with_publication_observations(
        publication_clock,
        request.action_allocation_start,
        request.action_allocation_snapshot_probe,
        request.action_allocation_previous_baseline_bytes,
        request.phase_allocation_probe,
    )
}

fn validate_request(request: &PreparedPpmlRequestV1, loaded: &LoadedPpmlInput) -> Result<()> {
    if request.bias.is_some() != loaded.bias_units.is_some() {
        return Err(invalid_argument(
            "bias request and independent-unit input disagree",
        ));
    }
    if let Some(bias) = request.bias {
        if request.ols.absorbed_terms.len() != 2
            || request
                .ols
                .absorbed_terms
                .iter()
                .any(|e| !e.intercept || !e.slope_sources.is_empty())
            || request
                .ols
                .model
                .terms
                .iter()
                .any(|t| t.raw_kind != crate::metadata::RawTermKind::Variable)
        {
            return Err(invalid_argument(
                "bias correction requires two categorical intercept effects and ordinary controls",
            ));
        }
        if bias.method == super::PpmlBiasMethod::Randomized
            && (bias.probes < 2 || bias.probes > u32::MAX as usize)
        {
            return Err(invalid_argument(
                "randomized bias correction requires 2 through 4294967295 probes",
            ));
        }
        if bias.method == super::PpmlBiasMethod::Exact
            && (bias.probes != 0 || bias.seed != 0 || bias.stream != 0)
        {
            return Err(invalid_argument(
                "exact bias correction does not accept randomization controls",
            ));
        }
        let units = loaded.bias_units.as_ref().unwrap();
        if units.values.len() != loaded.ols.outcome.values.len()
            || units.missing.len() != units.values.len()
        {
            return Err(invalid_argument(
                "bias unit input is not aligned on the original observation axis",
            ));
        }
    }
    validate_public_solver_request(request)?;
    request.ols.model.validate_executable_metadata()?;
    if !request.ols.include_intercept {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "prepared PPML requires the structural constant",
        ));
    }
    if request.clusters.len() > 5 {
        return Err(invalid_argument(
            "prepared PPML supports at most five cluster dimensions",
        ));
    }
    match (request.vce, request.clusters.is_empty()) {
        (PpmlVceRequest::Clustered, true) => {
            return Err(invalid_argument(
                "clustered PPML covariance requires declared cluster dimensions",
            ));
        }
        (PpmlVceRequest::Unadjusted | PpmlVceRequest::Robust, false) => {
            return Err(invalid_argument(
                "declared PPML cluster dimensions require clustered covariance",
            ));
        }
        _ => {}
    }
    match (loaded.adjustment_kind, loaded.adjustment.is_some()) {
        (PreparedPpmlAdjustmentKind::None, false)
        | (PreparedPpmlAdjustmentKind::Offset | PreparedPpmlAdjustmentKind::Exposure, true) => {}
        _ => {
            return Err(invalid_argument(
                "PPML adjustment kind and loaded adjustment column disagree",
            ));
        }
    }
    match (
        request.action_allocation_start,
        request.action_allocation_snapshot_probe,
        request.action_allocation_previous_baseline_bytes,
    ) {
        (None, None, None) | (Some(_), Some(_), _) => {}
        _ => {
            return Err(invalid_argument(
                "PPML action allocation observation requires a paired start snapshot and end probe",
            ));
        }
    }
    Ok(())
}

fn validate_public_solver_request(request: &PreparedPpmlRequestV1) -> Result<()> {
    if matches!(
        request.solver,
        PpmlSolverFamily::Auto | PpmlSolverFamily::Map
    ) {
        return Ok(());
    }
    if request.memory.requested == MemoryMode::Low {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "explicit PPML LSMR and CMG currently require memory(auto) or memory(fast)",
        ));
    }
    let fe_dimensions = request.ols.absorbed_terms.len();
    let topology_supported = match request.solver {
        PpmlSolverFamily::Lsmr => fe_dimensions >= 2,
        PpmlSolverFamily::Cmg => (2..=3).contains(&fe_dimensions),
        PpmlSolverFamily::Auto | PpmlSolverFamily::Map => true,
    };
    if !topology_supported {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            match request.solver {
                PpmlSolverFamily::Lsmr => {
                    "solver(lsmr) requires at least two categorical fixed-effect dimensions"
                }
                PpmlSolverFamily::Cmg => {
                    "solver(cmg) requires exactly two or three categorical fixed-effect dimensions"
                }
                PpmlSolverFamily::Auto | PpmlSolverFamily::Map => unreachable!(),
            },
        ));
    }
    if request.ols.absorbed_terms.iter().any(|term| {
        !term.intercept || !term.slope_sources.is_empty() || term.categorical_sources.is_empty()
    }) {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "explicit PPML LSMR and CMG require categorical intercept-only fixed effects",
        ));
    }
    if request.allocation_snapshot_probe.is_some() {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "the warmed PPML allocation audit is currently MAP-only",
        ));
    }
    Ok(())
}

struct BorrowedPpmlNumericInput<'a> {
    candidate: &'a [bool],
    outcome: NumericColumnView<'a>,
    numeric_sources: Vec<NumericColumnView<'a>>,
    source_uses: Vec<NumericSourceUse>,
    weights: WeightInput<'a>,
    offset: Option<NumericColumnView<'a>>,
    exposure: Option<NumericColumnView<'a>>,
}

impl BorrowedPpmlNumericInput<'_> {
    fn as_input(&self) -> PpmlNumericPreparationInput<'_> {
        PpmlNumericPreparationInput {
            candidate: self.candidate,
            outcome: self.outcome,
            numeric_sources: &self.numeric_sources,
            source_uses: &self.source_uses,
            weights: self.weights,
            offset: self.offset,
            exposure: self.exposure,
        }
    }
}

fn borrowed_numeric_input<'a>(
    request: &PreparedPpmlRequestV1,
    loaded: &'a LoadedPpmlInput,
) -> Result<BorrowedPpmlNumericInput<'a>> {
    if loaded.ols.numeric_sources.len() != request.ols.model.sources.len()
        || loaded
            .ols
            .numeric_sources
            .iter()
            .zip(&request.ols.model.sources)
            .any(|((loaded_id, _), source)| *loaded_id != source.id)
    {
        return Err(invalid_argument(
            "loaded PPML numeric sources are not in model-registry order",
        ));
    }
    let mut numeric_sources = Vec::new();
    numeric_sources
        .try_reserve_exact(loaded.ols.numeric_sources.len())
        .map_err(|_| allocation("unable to reserve PPML numeric source views"))?;
    for (_, column) in &loaded.ols.numeric_sources {
        numeric_sources.push(NumericColumnView::with_missing(
            &column.values,
            &column.missing,
        ));
    }
    let source_uses = source_uses(request)?;
    let weights = match (
        &loaded.ols.analytic_weight,
        &loaded.ols.frequency_weight,
        &loaded.ols.probability_weight,
    ) {
        (None, None, None) => WeightInput::None,
        (None, Some(column), None) => WeightInput::Provided {
            kind: WeightKind::Frequency,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        },
        (None, None, Some(column)) => WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        },
        (Some(_), None, None) => {
            return Err(FeregError::new(
                ErrorCode::UnsupportedWeight,
                "PPML does not support analytic weights",
            ));
        }
        _ => {
            return Err(invalid_argument(
                "loaded PPML input contains inconsistent weight columns",
            ));
        }
    };
    let adjustment = loaded
        .adjustment
        .as_ref()
        .map(|column| NumericColumnView::with_missing(&column.values, &column.missing));
    let (offset, exposure) = match loaded.adjustment_kind {
        PreparedPpmlAdjustmentKind::None => (None, None),
        PreparedPpmlAdjustmentKind::Offset => (adjustment, None),
        PreparedPpmlAdjustmentKind::Exposure => (None, adjustment),
    };
    Ok(BorrowedPpmlNumericInput {
        candidate: &loaded.ols.candidate,
        outcome: NumericColumnView::with_missing(
            &loaded.ols.outcome.values,
            &loaded.ols.outcome.missing,
        ),
        numeric_sources,
        source_uses,
        weights,
        offset,
        exposure,
    })
}

#[cfg(test)]
fn prepare_numeric_input(
    request: &PreparedPpmlRequestV1,
    loaded: &LoadedPpmlInput,
) -> Result<super::PreparedPpmlNumericData> {
    let borrowed = borrowed_numeric_input(request, loaded)?;
    prepare_ppml_numeric_data(&borrowed.as_input())
}

fn source_uses(request: &PreparedPpmlRequestV1) -> Result<Vec<NumericSourceUse>> {
    let mut uses = Vec::new();
    uses.try_reserve_exact(request.ols.model.sources.len())
        .map_err(|_| allocation("unable to reserve PPML numeric source-use plan"))?;
    for source in &request.ols.model.sources {
        let use_plan = NumericSourceUse {
            ordinary: request
                .ols
                .model
                .terms
                .iter()
                .flat_map(|term| &term.components)
                .any(|component| component.source == source.id),
            fixed_effect: request.ols.absorbed_terms.iter().any(|term| {
                term.categorical_sources
                    .iter()
                    .chain(&term.slope_sources)
                    .any(|candidate| *candidate == source.id)
            }),
            cluster: request
                .clusters
                .iter()
                .any(|term| term.sources.contains(&source.id)),
        };
        if !use_plan.ordinary && !use_plan.fixed_effect && !use_plan.cluster {
            return Err(invalid_argument(format!(
                "PPML source {} is unused by the model, absorb, and cluster recipes",
                source.id.get(),
            )));
        }
        uses.push(use_plan);
    }
    Ok(uses)
}

fn requested_saved_channel_count(request: &OlsRequestV1) -> Result<usize> {
    request
        .absorbed_terms
        .iter()
        .try_fold(0_usize, |total, term| {
            let channels = usize::from(term.intercept)
                .checked_add(term.slope_sources.len())
                .ok_or_else(|| allocation("PPML saved-channel count overflows"))?;
            total
                .checked_add(if term.saved_name.is_empty() {
                    0
                } else {
                    channels
                })
                .ok_or_else(|| allocation("PPML requested saved-channel count overflows"))
        })
}

fn requested_saved_channels(request: &OlsRequestV1) -> Result<Vec<bool>> {
    let count = request
        .absorbed_terms
        .iter()
        .try_fold(0_usize, |total, term| {
            total
                .checked_add(usize::from(term.intercept) + term.slope_sources.len())
                .ok_or_else(|| allocation("PPML saved-channel count overflows"))
        })?;
    let mut requested = Vec::new();
    requested
        .try_reserve_exact(count)
        .map_err(|_| allocation("unable to reserve PPML saved-channel request"))?;
    for term in &request.absorbed_terms {
        let save = !term.saved_name.is_empty();
        if term.intercept {
            requested.push(save);
        }
        requested.extend(std::iter::repeat_n(save, term.slope_sources.len()));
    }
    Ok(requested)
}

fn original_omission_statuses(request: &OlsRequestV1) -> Result<Vec<PpmlCoefficientStatus>> {
    let dimension = request
        .model
        .terms
        .len()
        .checked_add(usize::from(request.include_intercept))
        .ok_or_else(|| allocation("PPML original coefficient dimension overflows"))?;
    let mut statuses = Vec::new();
    statuses
        .try_reserve_exact(dimension)
        .map_err(|_| allocation("unable to reserve PPML original omission axis"))?;
    statuses.resize(dimension, PpmlCoefficientStatus::SyntaxOmitted);
    for term in &request.model.terms {
        let index = usize::try_from(term.coefficient_slot.get() - 1)
            .map_err(|_| invalid_argument("PPML coefficient slot does not fit usize"))?;
        let target = statuses
            .get_mut(index)
            .ok_or_else(|| invalid_argument("PPML coefficient slot is outside its model graph"))?;
        *target = if term.whole_column_omitted
            && term
                .components
                .iter()
                .any(|component| component.base == crate::metadata::MetadataFlag::True)
        {
            PpmlCoefficientStatus::BaseLevel
        } else {
            PpmlCoefficientStatus::SyntaxOmitted
        };
    }
    Ok(statuses)
}

fn cluster_code_views(prepared: &PreparedPpmlModelGeneration) -> Result<Vec<&[u32]>> {
    let dimensions = prepared.cluster_design().map_or(0, |design| design.len());
    let mut output = Vec::new();
    output
        .try_reserve_exact(dimensions)
        .map_err(|_| allocation("unable to reserve PPML cluster code views"))?;
    for dimension in 0..dimensions {
        output.push(prepared.cluster_codes(dimension)?);
    }
    Ok(output)
}

/// Conservative dimensions re-derived after numeric preparation. This exact
/// shape is used only to validate and bind the already-selected route.
#[cfg(test)]
fn conservative_preflight_dimensions(
    request: &PreparedPpmlRequestV1,
    numeric: &super::prepared::PreparedPpmlNumericData,
    requested_saved_fe_channels: usize,
) -> Result<PpmlMemoryDimensions> {
    conservative_preflight_dimensions_from_classification(
        request,
        PpmlNumericClassification {
            n_original: numeric.sample().n_original,
            n_candidate: numeric.sample().n_candidate,
            n_active: numeric.sample().n_active(),
        },
        numeric.sample().regressors.len(),
        numeric.adjustment().input_kind != PreparedPpmlAdjustmentKind::None,
        numeric.weights().kind == Some(WeightKind::Probability),
        requested_saved_fe_channels,
    )
}

/// Distinct category tuples cannot outnumber consecutive tuple runs, in any
/// input order. Counting all loaded rows also bounds every subsequent sample
/// restriction. Only borrowed, model-sized column views are allocated here;
/// no group IDs, row masks, or observation-sized preparation state are built.
fn refine_preflight_fe_bounds(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
    dimensions: &mut PpmlMemoryDimensions,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<()> {
    let n = dimensions.observations;
    let mut total_levels = 0_usize;
    let mut max_levels = 0_usize;
    let mut total_directions = 0_usize;
    let mut max_directions = 0_usize;
    for term in &request.absorbed_terms {
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(term.categorical_sources.len())
            .map_err(|_| allocation("unable to reserve PPML FE bound source views"))?;
        for source in &term.categorical_sources {
            let column = loaded
                .numeric_sources
                .iter()
                .find(|(id, _)| id == source)
                .ok_or_else(|| invalid_argument("PPML FE bound source is missing"))?;
            columns.push(column.1.values.as_slice());
        }
        let mut runs = usize::from(n != 0);
        for start in (1..loaded.outcome.values.len()).step_by(65_536) {
            interrupt(PpmlInterruptPoint::Preparation)?;
            if runs >= n {
                break;
            }
            let end = start
                .saturating_add(65_536)
                .min(loaded.outcome.values.len());
            for row in start..end {
                if columns.iter().any(|column| column[row] != column[row - 1]) {
                    runs = runs
                        .checked_add(1)
                        .ok_or_else(|| allocation("PPML FE run count overflows"))?;
                    if runs >= n {
                        break;
                    }
                }
            }
        }
        let mut levels = runs.min(n);
        if n >= 4_096 && levels > n / 4 {
            let mut product = 1_usize;
            for column in &columns {
                let Some(distinct) = bounded_integer_level_count(column, n, interrupt)? else {
                    product = n;
                    break;
                };
                product = product.saturating_mul(distinct).min(n);
            }
            levels = levels.min(product);
        }
        let channels = usize::from(term.intercept)
            .checked_add(term.slope_sources.len())
            .ok_or_else(|| allocation("PPML FE bound channel count overflows"))?;
        let directions = levels
            .checked_mul(channels)
            .ok_or_else(|| allocation("PPML FE bound direction count overflows"))?;
        total_levels = total_levels
            .checked_add(levels)
            .ok_or_else(|| allocation("PPML FE bound total levels overflow"))?;
        total_directions = total_directions
            .checked_add(directions)
            .ok_or_else(|| allocation("PPML FE bound total directions overflow"))?;
        max_levels = max_levels.max(levels);
        max_directions = max_directions.max(directions);
    }
    dimensions.fe_total_levels = total_levels;
    dimensions.fe_max_levels = max_levels;
    dimensions.fe_coefficient_directions = total_directions;
    dimensions.fe_max_effect_directions = max_directions;
    Ok(())
}

const PREFLIGHT_INTEGER_BITMAP_WORDS: usize = 32_768;
const PREFLIGHT_INTEGER_WINDOW: usize = PREFLIGHT_INTEGER_BITMAP_WORDS * 64;
const PREFLIGHT_INTEGER_MAX_PASSES: usize = 128;

/// Exact distinct integers with a fixed 256 KiB stack bitmap. Each pass covers
/// a disjoint value interval, so neither hashing nor a statistical cardinality
/// estimate can under-admit memory. The pass ceiling keeps the work linear in
/// N even for adversarial category values. Other domains retain the run bound.
#[inline(never)]
fn bounded_integer_level_count(
    values: &[f64],
    cap: usize,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<Option<usize>> {
    if values.is_empty() || cap == 0 {
        return Ok(Some(0));
    }
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for chunk in values.chunks(65_536) {
        interrupt(PpmlInterruptPoint::Preparation)?;
        for &value in chunk {
            if !value.is_finite() || value.abs() > 4_503_599_627_370_496.0 || value.fract() != 0.0 {
                return Ok(None);
            }
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }
    }
    let span = (maximum - minimum) as u64;
    let passes = span / PREFLIGHT_INTEGER_WINDOW as u64 + 1;
    if passes > PREFLIGHT_INTEGER_MAX_PASSES as u64 {
        return Ok(None);
    }
    let mut bitmap = [0_u64; PREFLIGHT_INTEGER_BITMAP_WORDS];
    let mut count = 0_usize;
    for pass in 0..passes {
        bitmap.fill(0);
        let lower = minimum + (pass * PREFLIGHT_INTEGER_WINDOW as u64) as f64;
        for chunk in values.chunks(65_536) {
            interrupt(PpmlInterruptPoint::Preparation)?;
            for &value in chunk {
                let position = value - lower;
                if position >= 0.0 && position < PREFLIGHT_INTEGER_WINDOW as f64 {
                    let position = position as usize;
                    bitmap[position / 64] |= 1_u64 << (position % 64);
                }
            }
        }
        count = count.saturating_add(
            bitmap
                .iter()
                .map(|word| word.count_ones() as usize)
                .sum::<usize>(),
        );
        if count >= cap {
            return Ok(Some(cap));
        }
    }
    Ok(Some(count))
}

/// Allocation-bounded conservative dimensions available after authoritative
/// row classification but before canonical numeric compaction, original-axis
/// ledgers, categorical topology, or the generation-zero owner exist.
fn conservative_preflight_dimensions_from_classification(
    request: &PreparedPpmlRequestV1,
    numeric: PpmlNumericClassification,
    loaded_numeric_columns: usize,
    adjusted: bool,
    probability_weighted: bool,
    requested_saved_fe_channels: usize,
) -> Result<PpmlMemoryDimensions> {
    let observations = numeric.n_active;
    // This is a conservative pre-owner reservation boundary, not a second
    // compilation authority.  Counting every expanded term plus a requested
    // intercept safely dominates the final compact columns and reported
    // coefficient slots, while avoiding an infallible deep clone of the
    // complete model and cluster recipes solely to call the borrowed compiler.
    let conservative_coefficient_slots = request
        .ols
        .model
        .terms
        .len()
        .checked_add(usize::from(request.ols.include_intercept))
        .ok_or_else(|| allocation("PPML preflight coefficient slots overflow"))?;
    let fe_dimensions = request.ols.absorbed_terms.len();
    let mut fe_total_channels = 0_usize;
    let mut fe_max_channels = 0_usize;
    for effect in &request.ols.absorbed_terms {
        let channels = usize::from(effect.intercept)
            .checked_add(effect.slope_sources.len())
            .ok_or_else(|| allocation("PPML preflight FE channels overflow"))?;
        fe_total_channels = fe_total_channels
            .checked_add(channels)
            .ok_or_else(|| allocation("PPML preflight total FE channels overflow"))?;
        fe_max_channels = fe_max_channels.max(channels);
    }
    let fe_total_levels = observations
        .checked_mul(fe_dimensions)
        .ok_or_else(|| allocation("PPML preflight FE levels overflow"))?;
    let fe_coefficient_directions = observations
        .checked_mul(fe_total_channels)
        .ok_or_else(|| allocation("PPML preflight FE directions overflow"))?;
    let fe_max_effect_directions = observations
        .checked_mul(fe_max_channels)
        .ok_or_else(|| allocation("PPML preflight maximum FE directions overflow"))?;
    let subset_count = 1_usize
        .checked_shl(request.clusters.len() as u32)
        .and_then(|count| count.checked_sub(1))
        .ok_or_else(|| allocation("PPML preflight cluster subset count overflow"))?;
    let cluster_subset_groups = observations
        .checked_mul(subset_count)
        .ok_or_else(|| allocation("PPML preflight cluster groups overflow"))?;
    let separation_workspace = separation_workspace_requirements(request.separation);
    Ok(PpmlMemoryDimensions {
        metadata: super::PpmlMetadataMemory::from_request(&request.ols, &request.clusters)?,
        observations,
        original_observations: numeric.n_original,
        candidate_observations: numeric.n_candidate,
        loaded_numeric_columns,
        retained_base_adjustment_capacity_elements: usize::from(adjusted) * numeric.n_candidate,
        // The owner copies raw pweights only for generation-zero canonical
        // rows. Missing/domain-excluded if/in candidates have no reusable
        // weight, while separation cannot shrink this retained base vector.
        retained_raw_preseparation_probability_weight_elements: usize::from(probability_weighted)
            * observations,
        finite_columns: conservative_coefficient_slots,
        coefficient_slots: conservative_coefficient_slots,
        reported_coefficient_slots: conservative_coefficient_slots,
        fe_dimensions,
        fe_total_levels,
        fe_max_levels: if fe_dimensions == 0 { 0 } else { observations },
        fe_total_channels,
        fe_max_channels,
        fe_max_level_members: if fe_dimensions == 0 { 0 } else { observations },
        fe_coefficient_directions,
        fe_max_effect_directions,
        cluster_dimensions: request.clusters.len(),
        cluster_subset_groups,
        separation_workspace_vectors: separation_workspace.observation_vectors,
        separation_singleton_enabled: request.separation.drop_singletons,
        separation_method_count: separation_method_count(request.separation),
        separation_generation_capacity: observations,
        separation_rebase_enabled: separation_can_rebase(request.separation),
        separation_exact_fe_enabled: request.separation.methods.fixed_effects,
        separation_mu_enabled: request.separation.methods.mu,
        simplex_tableau_cells: separation_workspace.simplex_tableau_cells,
        requested_output_columns: request.output.requested_observation_columns,
        requested_mean_output: request.output.mean,
        requested_saved_fe_channels,
        publication_matrix_cells: request.output.publication_matrix_cells,
        threads: request.linear.execution.threads.resolved(),
        allocation_audit_enabled: request.allocation_snapshot_probe.is_some(),
    })
}

fn admit_load_hint(
    hint: Option<PreparedOlsLoadHint>,
    classified: PpmlNumericClassification,
    metadata: super::PpmlMetadataMemory,
) -> Option<PreparedOlsLoadHint> {
    // Before generation-zero topology, its separately charged row bank is
    // unallocated. The capsule fits only there, beside selection/numeric
    // copies; adoption consumes it before ordinary topology construction.
    hint.filter(|hint| {
        classified.n_active == classified.n_original
            && (classified.n_active as u128)
                .checked_mul(metadata.generation_bytes_per_row)
                .is_some_and(|bound| hint.adoption_workspace_bytes() <= bound)
    })
}

/// Count capacities of the owner transferred to the separation coordinator.
/// This descriptor-only walk allocates nothing and reads no observation values.
fn raw_input_owner_bytes(request: &PreparedPpmlRequestV1, loaded: &LoadedPpmlInput) -> u128 {
    use crate::memory::vec_capacity_bytes as bytes;
    let column = |value: &LoadedNumericColumn| bytes(&value.values) + bytes(&value.missing);
    request.ols.owned_buffer_bytes()
        + bytes(&request.clusters)
        + request
            .clusters
            .iter()
            .map(|cluster| cluster.label.capacity() as u128 + bytes(&cluster.sources))
            .sum::<u128>()
        + bytes(&loaded.ols.candidate)
        + column(&loaded.ols.outcome)
        + bytes(&loaded.ols.numeric_sources)
        + loaded
            .ols
            .numeric_sources
            .iter()
            .map(|(_, value)| column(value))
            .sum::<u128>()
        + [
            &loaded.ols.analytic_weight,
            &loaded.ols.frequency_weight,
            &loaded.ols.probability_weight,
            &loaded.adjustment,
            &loaded.bias_units,
        ]
        .into_iter()
        .flatten()
        .map(column)
        .sum::<u128>()
}

// Keep the legacy workspace and ownership boundaries explicit during migration.
#[allow(clippy::too_many_arguments)]
fn final_memory_dimensions(
    prepared: &PreparedPpmlModelGeneration,
    clusters: &[&[u32]],
    separation: SeparationConfig,
    output: PreparedPpmlOutputRequest,
    requested_saved_fe_channels: usize,
    linear: PreparedOlsFitConfig,
    allocation_audit_enabled: bool,
    metadata: super::PpmlMetadataMemory,
) -> Result<PpmlMemoryDimensions> {
    let categorical = prepared.prepared().as_ref();
    let actual_nested_bytes = categorical.nested_metadata_capacity_bytes()
        + prepared
            .cluster_design()
            .map_or(0, |clusters| clusters.metadata_capacity_bytes());
    if actual_nested_bytes > metadata.prepared_nested_bytes {
        return Err(allocation(
            "PPML prepared metadata capacities exceed generation-zero admission",
        ));
    }
    let topology = categorical.fe_topology();
    let specs = categorical.effect_specs();
    if specs.len() != topology.effect_to_topology().len() {
        return Err(invalid_argument(
            "prepared PPML effect metadata and topology are misaligned",
        ));
    }
    let mut total_levels = 0_usize;
    let mut max_levels = 0_usize;
    let mut total_channels = 0_usize;
    let mut max_channels = 0_usize;
    let mut max_level_members = 0_usize;
    let mut directions = 0_usize;
    let mut max_directions = 0_usize;
    for (effect, &source_index) in specs.iter().zip(topology.effect_to_topology()) {
        let source = topology.registry().source(source_index)?;
        let levels = source.n_levels();
        let channels = usize::from(effect.intercept) + effect.slopes.len();
        let effect_directions = levels
            .checked_mul(channels)
            .ok_or_else(|| allocation("PPML FE coefficient directions overflow"))?;
        total_levels = total_levels
            .checked_add(levels)
            .ok_or_else(|| allocation("PPML total FE levels overflow"))?;
        total_channels = total_channels
            .checked_add(channels)
            .ok_or_else(|| allocation("PPML total FE channels overflow"))?;
        directions = directions
            .checked_add(effect_directions)
            .ok_or_else(|| allocation("PPML total FE directions overflow"))?;
        max_levels = max_levels.max(levels);
        max_channels = max_channels.max(channels);
        max_directions = max_directions.max(effect_directions);
        for level in 0..levels {
            max_level_members = max_level_members.max(source.members(level)?.len());
        }
    }
    let cluster_subset_groups = cluster_subset_groups(clusters)?;
    let separation_workspace = separation_workspace_requirements(separation);
    Ok(PpmlMemoryDimensions {
        metadata,
        observations: prepared.original_rows().len(),
        original_observations: prepared.original_observations(),
        // The owner retains the generation-zero if/in capacity even though
        // each rebuilt OLS sample reports only that generation's staged
        // candidate mask. Route binding therefore uses the immutable numeric
        // candidate count, never the shrinking generation-local count.
        candidate_observations: prepared.requested_candidate_observations(),
        loaded_numeric_columns: prepared.prepared().sample().regressors.len(),
        retained_base_adjustment_capacity_elements: prepared
            .retained_base_adjustment_capacity_elements(),
        retained_raw_preseparation_probability_weight_elements: prepared
            .retained_raw_preseparation_probability_weight_elements(),
        finite_columns: prepared.model().columns.len(),
        coefficient_slots: prepared.model().coefficients.len(),
        reported_coefficient_slots: categorical.report_plan().original_coefficient_count(),
        fe_dimensions: specs.len(),
        fe_total_levels: total_levels,
        fe_max_levels: max_levels,
        fe_total_channels: total_channels,
        fe_max_channels: max_channels,
        fe_max_level_members: max_level_members,
        fe_coefficient_directions: directions,
        fe_max_effect_directions: max_directions,
        cluster_dimensions: clusters.len(),
        cluster_subset_groups,
        separation_workspace_vectors: separation_workspace.observation_vectors,
        separation_singleton_enabled: separation.drop_singletons,
        separation_method_count: separation_method_count(separation),
        separation_generation_capacity: separation_generation_capacity(
            prepared.original_rows().len(),
            prepared.outcome().iter().all(|value| *value > 0.0),
            separation.drop_singletons,
        ),
        separation_rebase_enabled: separation_can_rebase(separation),
        separation_exact_fe_enabled: separation.methods.fixed_effects,
        separation_mu_enabled: separation.methods.mu,
        simplex_tableau_cells: separation_workspace.simplex_tableau_cells,
        requested_output_columns: output.requested_observation_columns,
        requested_mean_output: output.mean,
        requested_saved_fe_channels,
        publication_matrix_cells: output.publication_matrix_cells,
        threads: linear.execution.threads.resolved(),
        allocation_audit_enabled,
    })
}

const fn separation_can_rebase(config: SeparationConfig) -> bool {
    config.drop_singletons
        || config.methods.fixed_effects
        || config.methods.simplex
        || config.methods.iterated_rectifier
        || config.methods.mu
}

fn cluster_subset_groups(clusters: &[&[u32]]) -> Result<usize> {
    if clusters.is_empty() {
        return Ok(0);
    }
    let rows = clusters[0].len();
    if rows == 0 || clusters.iter().any(|codes| codes.len() != rows) {
        return Err(invalid_argument(
            "PPML cluster dimensions are empty or not final-sample aligned",
        ));
    }
    // This dimension is a preflight capacity, not a second topology build.
    // Canonical codes make max(code)+1 the exact marginal group count.  A
    // joint subset can have at most the product of its marginal counts and at
    // most N observed tuples, so summing those minima is allocation-free and
    // provably never below the exact shared-sandwich group allocation.
    let mut marginal_groups = Vec::new();
    marginal_groups
        .try_reserve_exact(clusters.len())
        .map_err(|_| allocation("unable to reserve PPML marginal cluster counts"))?;
    for codes in clusters {
        let groups = codes
            .iter()
            .copied()
            .max()
            .and_then(|maximum| usize::try_from(maximum).ok())
            .and_then(|maximum| maximum.checked_add(1))
            .ok_or_else(|| invalid_argument("PPML cluster codes are not representable"))?;
        if groups < 2 {
            return Err(FeregError::new(
                ErrorCode::InsufficientObservations,
                "PPML cluster dimension has fewer than two groups",
            ));
        }
        marginal_groups.push(groups);
    }
    let subset_count = (1_usize << clusters.len()) - 1;
    let mut total = 0_usize;
    for mask in 1..=subset_count {
        let mut product = 1_usize;
        for (dimension, groups) in marginal_groups.iter().copied().enumerate() {
            if mask & (1_usize << dimension) != 0 {
                product = product.checked_mul(groups).unwrap_or(rows);
                if product >= rows {
                    product = rows;
                    break;
                }
            }
        }
        let groups = product.min(rows);
        total = total
            .checked_add(groups)
            .ok_or_else(|| allocation("PPML cluster subset group count overflows"))?;
    }
    Ok(total)
}

fn map_coefficients(
    model: &PreparedPpmlModel,
    fit: &PreparedPpmlIrlsFit,
    original_omissions: &[PpmlCoefficientStatus],
) -> Result<PpmlCoefficientOutput> {
    let fitted = fit.coefficients();
    let fitted_dimension = model.model().coefficients.len();
    if fitted.len() != fitted_dimension {
        return Err(invalid_argument(
            "prepared PPML fitted coefficients do not match final model slots",
        ));
    }
    let report_plan = model.report_plan();
    let dimension = report_plan.original_coefficient_count();
    if original_omissions.len() != dimension {
        return Err(invalid_argument(
            "prepared PPML omission metadata does not span the original coefficient axis",
        ));
    }
    let mut values = fallible_filled(dimension, 0.0, "unable to reserve PPML coefficient values")?;
    let mut statuses = fallible_copy(
        original_omissions,
        "unable to copy PPML coefficient statuses",
    )?;
    let mut generated = fallible_filled(
        dimension,
        None,
        "unable to reserve PPML generated-column mapping",
    )?;
    for coefficient in fitted {
        let original_slot = report_plan.original_slot(coefficient.coefficient_slot)?;
        let slot = usize::try_from(original_slot.get() - 1).map_err(|_| {
            invalid_argument("prepared PPML original coefficient slot does not fit usize")
        })?;
        if slot >= dimension {
            return Err(invalid_argument(
                "prepared PPML coefficient slot is out of final-model range",
            ));
        }
        generated[slot] = coefficient.generated_column_index;
        statuses[slot] = match coefficient.status {
            PreparedOlsFitStatus::Kept => PpmlCoefficientStatus::Identified,
            PreparedOlsFitStatus::SyntaxOmitted => original_omissions[slot],
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept => {
                PpmlCoefficientStatus::NormalizedStructuralConstant
            }
            PreparedOlsFitStatus::CollinearWithFixedEffects => {
                PpmlCoefficientStatus::CollinearWithFixedEffects
            }
            PreparedOlsFitStatus::NumericallyRankDeficient => {
                PpmlCoefficientStatus::NumericallyRankDeficient
            }
            PreparedOlsFitStatus::AbsorbedInterceptDeferred => {
                return Err(invalid_argument(
                    "prepared PPML final coefficients retain a deferred intercept",
                ));
            }
        };
        // A structural normalization contributes one reported rank direction
        // but is deliberately outside the compact finite design.  Its public
        // coefficient convention is therefore status 7 with an exact zero,
        // never the arbitrary representative returned by an absorbed solve.
        values[slot] = if statuses[slot] == PpmlCoefficientStatus::NormalizedStructuralConstant {
            0.0
        } else {
            coefficient.value.unwrap_or(0.0)
        };
    }
    let mut kept_full_slots = Vec::new();
    kept_full_slots
        .try_reserve_exact(fit.linear().basis().kept_generated_columns().len())
        .map_err(|_| allocation("unable to reserve PPML compact coefficient mapping"))?;
    for &generated_column in fit.linear().basis().kept_generated_columns() {
        let column = model
            .model()
            .columns
            .get(generated_column)
            .ok_or_else(|| invalid_argument("PPML frozen basis column is out of range"))?;
        let original_slot = report_plan.original_slot(column.coefficient_slot)?;
        kept_full_slots.push(usize::try_from(original_slot.get() - 1).map_err(|_| {
            invalid_argument("PPML kept original coefficient slot does not fit usize")
        })?);
    }
    PpmlCoefficientOutput::new(values, statuses, generated, kept_full_slots)
}

fn wald_columns(
    model: &PreparedPpmlModel,
    coefficients: &PpmlCoefficientOutput,
    fit: &PreparedPpmlIrlsFit,
) -> Result<(Vec<usize>, Option<super::PpmlWaldReference>)> {
    if let Some(constant) = fit.linear().constant_representation() {
        return constant_wald_columns(coefficients.kept_full_slots(), constant);
    }
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(coefficients.kept_full_slots().len())
        .map_err(|_| allocation("unable to reserve PPML Wald columns"))?;
    for &slot in coefficients.kept_full_slots() {
        let generated = coefficients
            .generated_columns()
            .get(slot)
            .copied()
            .flatten()
            .ok_or_else(|| invalid_argument("identified PPML coefficient lacks a column recipe"))?;
        if !matches!(
            model
                .model()
                .columns
                .get(generated)
                .ok_or_else(|| invalid_argument("PPML Wald column recipe is out of range"))?
                .kind,
            PreparedOlsColumnKind::Intercept
        ) {
            columns.push(slot);
        }
    }
    Ok((columns, None))
}

#[allow(clippy::too_many_arguments)]
fn recover_saved_effects(
    model: &PreparedPpmlModel,
    fit: &PreparedPpmlIrlsFit,
    prior_weights: &[f64],
    report: PreparedPpmlFixedEffectReport,
    requested: &[bool],
    linear: PreparedOlsFitConfig,
    tolerance: f64,
    inject_allocation_failure: bool,
) -> Result<(
    Vec<super::PpmlFixedEffectResult>,
    Option<super::PpmlSavedEffectOutput>,
)> {
    if report.rows.len() != requested.len() {
        return Err(invalid_argument(
            "PPML saved-FE report and request masks are misaligned",
        ));
    }
    let mut lsmr = linear.absorb.lsmr;
    lsmr.tolerance = lsmr.tolerance.min(tolerance);
    let mut map = linear.absorb.map;
    map.tolerance = map.tolerance.min(tolerance);
    let saves_requested = requested.iter().any(|&saved| saved);
    let effective_threads = fit.diagnostics().memory.effective_threads;
    let recover = || {
        if saves_requested && rayon::current_num_threads() != effective_threads {
            return Err(FeregError::new(
                ErrorCode::Internal,
                "PPML FE recovery worker count differs from memory admission",
            ));
        }
        recover_ppml_fixed_effects(
            model,
            report,
            fit.linear().weights(),
            // These independent inputs verify the explicit weights are q*mu.
            prior_weights,
            fit.mean(),
            fit.linear().absorbed_predictor(),
            None,
            lsmr,
            map,
            inject_allocation_failure,
        )
    };
    let recovered = if saves_requested {
        // Recovery uses Rayon for CG and normalization. Keep it in the
        // admitted fit's team; the process-global default can have a different
        // size and allocate a second registry after fitting has completed.
        let mut threads = linear.execution.threads;
        threads.requested = std::num::NonZeroUsize::new(effective_threads);
        crate::parallel::ThreadTeam::new(threads).install(recover)??
    } else {
        recover()?
    };
    if recovered.reconstruction_relative_infinity_norm > tolerance {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML saved-FE recovery exceeds the publication tolerance",
        ));
    }
    Ok((recovered.report.rows, recovered.saved_effects))
}

fn convergence_report(fit: &PreparedPpmlIrlsFit) -> Result<PpmlConvergenceReport> {
    let diagnostics = fit.diagnostics();
    let linear = fit.linear();
    Ok(PpmlConvergenceReport {
        converged: diagnostics.converged,
        two_consecutive_convergence: diagnostics.two_consecutive_convergence,
        rank_frozen: diagnostics.rank_frozen,
        outer_iterations: diagnostics.iterations,
        inner_iterations: diagnostics.total_inner_iterations,
        step_halvings: diagnostics.total_step_halvings,
        strict_profile_rejections: diagnostics.strict_profile_rejections,
        strict_final_solve_passed: linear
            .frame()
            .canonical_proof()
            .is_some_and(|proof| proof.rank.passed && proof.score.passed && proof.bread.passed),
        linear_within_relative_infinity_norm: linear
            .within_certificate()
            .left_inverse_relative_infinity_norm
            .max(
                linear
                    .within_certificate()
                    .right_inverse_relative_infinity_norm,
            ),
        linear_reconstruction_relative_infinity_norm: linear
            .reconstruction_certificate()
            .relative_infinity_norm,
        final_score: diagnostics.final_score.clone(),
    })
}

fn cluster_metadata(
    prepared: &PreparedPpmlModelGeneration,
) -> Result<Vec<PpmlClusterDimensionResult>> {
    let Some(design) = prepared.cluster_design() else {
        return Ok(Vec::new());
    };
    let mut output = Vec::new();
    output
        .try_reserve_exact(design.len())
        .map_err(|_| allocation("unable to reserve PPML cluster metadata"))?;
    for (declared_dimension, dimension) in design.dimensions().iter().enumerate() {
        let mut source_ids = Vec::new();
        source_ids
            .try_reserve_exact(dimension.sources().len())
            .map_err(|_| allocation("unable to reserve PPML cluster source identities"))?;
        for source in dimension.sources() {
            source_ids.push(source.get());
        }
        output.push(PpmlClusterDimensionResult {
            declared_dimension,
            label: fallible_string_copy(dimension.label(), "unable to copy PPML cluster label")?,
            source_ids,
            groups: dimension.groups(),
        });
    }
    Ok(output)
}

fn record_separation_timings(
    memory: &mut super::PpmlMemoryTelemetry,
    report: &crate::ppml_separation::SeparationReport,
) -> Result<()> {
    for method in &report.methods {
        if !method.elapsed_seconds.is_finite() || method.elapsed_seconds < 0.0 {
            return Err(invalid_argument(
                "prepared PPML separation timing is not finite and nonnegative",
            ));
        }
        let phase = match method.method {
            SeparationMethod::Singletons | SeparationMethod::FixedEffects => {
                PpmlPhase::SeparationFe
            }
            SeparationMethod::Simplex => PpmlPhase::SeparationSimplex,
            SeparationMethod::IteratedRectifier => PpmlPhase::SeparationIr,
            SeparationMethod::Mu => PpmlPhase::SeparationMu,
        };
        memory.record_phase(phase, Duration::from_secs_f64(method.elapsed_seconds));
    }
    Ok(())
}

fn fallible_string_copy(source: &str, message: &'static str) -> Result<String> {
    let mut output = fallible_string_with_capacity(source.len(), message)?;
    output.push_str(source);
    Ok(output)
}

fn fallible_string_with_capacity(length: usize, message: &'static str) -> Result<String> {
    let mut output = String::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| allocation(message))?;
    Ok(output)
}

#[cfg(test)]
mod tests;
