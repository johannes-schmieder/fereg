//! One owned, certified production boundary for prepared PPML.
//!
//! This module is deliberately the only composition layer that is allowed to
//! join numeric preparation, categorical topology, separation, the reusable
//! weighted-linear session, PPML inference, and publication metadata.  It
//! never calls the legacy dense PPML fitter or its legacy covariance path.

use std::time::Duration;

use crate::data::{NumericColumnView, NumericSourceUse, WeightInput, WeightKind};
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeSolverKind;
use crate::ols::{
    LinearSessionPhaseAllocatorProbe, LoadedNumericColumn, LoadedOlsInput, OlsClusterTermV1,
    OlsRequestV1, PreparedOlsColumnKind, PreparedOlsFitConfig, PreparedOlsFitStatus,
};
use crate::options::MemoryMode;
use crate::ppml_separation::{
    separation_generation_capacity, separation_method_count, separation_workspace_requirements,
    SeparationConfig, SeparationMethod,
};

use super::memory::{begin_phase_allocation, end_phase_allocation, plan_ppml_memory_with_probe};
use super::prepared::{
    classify_ppml_numeric_data, select_ppml_numeric_data, PpmlNumericClassification,
    PpmlNumericPreparationInput, PreparedPpmlAdjustmentKind,
};
use super::prepared_irls::{
    explore_prepared_ppml_mu_interruptible, fit_prepared_ppml_irls_interruptible,
    PreparedPpmlIrlsConfig, PreparedPpmlIrlsFit,
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
    pub ols: LoadedOlsInput,
    pub adjustment: Option<LoadedNumericColumn>,
    pub adjustment_kind: PreparedPpmlAdjustmentKind,
}

/// Caller-thread safe points exposed by the interruptible PPML entry.
///
/// The callback is deliberately accepted without `Send` or `Sync`: embeddings
/// such as Stata may poll only on the thread that entered the native plugin.
/// Long parallel kernels therefore poll immediately before or after their
/// caller-thread boundary, never from a Rayon worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmlInterruptPoint {
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
    loaded: LoadedPpmlInput,
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<CertifiedPpmlResult> {
    validate_request(&request, &loaded)?;
    let mut solver_plan = PpmlSolverPlan::new(request.solver);
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

    // Build only borrowed column views and model-sized role metadata, then
    // classify the numeric sample without constructing any observation-sized
    // compact column. ADR 0018 freezes route selection at this boundary.
    let numeric_input = borrowed_numeric_input(&request, &loaded)?;
    let numeric_classification = classify_ppml_numeric_data(&numeric_input.as_input())?;
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
    refine_preflight_fe_bounds(
        &request.ols,
        &loaded.ols,
        &mut preflight_dimensions,
        interrupt,
    )?;
    let preliminary_plan = plan_ppml_memory_with_probe(
        preflight_dimensions,
        request.memory,
        request.memory_preflight_probe,
    )?;
    if request.solver == PpmlSolverFamily::Auto
        && auto_lsmr_eligible(&request)
        && preliminary_plan.selected == PpmlMemoryRoute::Fast
    {
        solver_plan.selected = PpmlSolverFamily::Lsmr;
    }

    begin_phase_allocation(request.phase_allocation_probe, PpmlPhase::Preparation);
    let numeric = select_ppml_numeric_data(&numeric_input.as_input(), numeric_classification)?;
    // Selection has checked the exact classified shape. Until topology exists,
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
    )?;
    let preparation_elapsed = preparation_clock.elapsed();
    let preparation_peak =
        end_phase_allocation(request.phase_allocation_probe, PpmlPhase::Preparation);
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
    let (separation_outcome, fit) = fit_with_prepared_ppml_separation(
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
            )?;
            let final_plan = (*preliminary_plan).with_final_dimensions(dimensions)?;
            explore_prepared_ppml_mu_interruptible(
                generation.fit_view(),
                generation.outcome(),
                generation.prior_weights(),
                generation.offset(),
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
                separation.mu_tolerance,
                interrupt,
            )
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
            )?;
            let final_plan = (*preliminary_plan).with_final_dimensions(dimensions)?;
            let fit = fit_prepared_ppml_irls_interruptible(
                generation.fit_view(),
                generation.outcome(),
                generation.prior_weights(),
                generation.offset(),
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
                interrupt,
            )?;
            Ok(fit)
        },
    )?;
    let final_drop_reasons = separation_outcome.final_drop_reasons()?;
    let prepared = &separation_outcome.prepared;
    let fixed_effect_report =
        prepare_ppml_fixed_effect_report(prepared.prepared(), &requested_saved_channels)?;
    let cluster_codes = cluster_code_views(prepared)?;

    let coefficients = map_coefficients(prepared.prepared(), &fit, &omission_statuses)?;
    let wald_columns = wald_columns(prepared.prepared(), &coefficients)?;
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
    let sample = PpmlFinalSample::new(
        prepared.original_observations(),
        prepared.requested_candidate_observations(),
        separation_outcome.candidate_original_rows.clone(),
        fallible_copy(
            prepared.original_rows(),
            "unable to copy PPML final-sample row ledger",
        )?,
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
        fallible_copy(
            prepared.original_rows(),
            "unable to copy PPML output row ledger",
        )?,
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
    memory.record_phase(PpmlPhase::Preparation, preparation_elapsed);
    if let Some(bytes) = preparation_peak {
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
    let result = CertifiedPpmlResult::new(CertifiedPpmlResultInput {
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
    })?
    .retain_requested_outputs();
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

fn auto_lsmr_eligible(request: &PreparedPpmlRequestV1) -> bool {
    // The frozen calibration admitted two-/three-way categorical designs,
    // not every topology accepted by the explicit LSMR family.
    (2..=3).contains(&request.ols.absorbed_terms.len())
        && request.memory.requested != MemoryMode::Low
        && request.allocation_snapshot_probe.is_none()
        && request.ols.absorbed_terms.iter().all(|term| {
            term.intercept && term.slope_sources.is_empty() && !term.categorical_sources.is_empty()
        })
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
        *target = if term
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

fn final_memory_dimensions(
    prepared: &PreparedPpmlModelGeneration,
    clusters: &[&[u32]],
    separation: SeparationConfig,
    output: PreparedPpmlOutputRequest,
    requested_saved_fe_channels: usize,
    linear: PreparedOlsFitConfig,
    allocation_audit_enabled: bool,
) -> Result<PpmlMemoryDimensions> {
    let categorical = prepared
        .prepared()
        .categorical()
        .ok_or_else(|| invalid_argument("prepared PPML generation lacks categorical topology"))?;
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
    let report_plan = model
        .categorical()
        .ok_or_else(|| invalid_argument("prepared PPML coefficient map lacks report plan"))?
        .report_plan();
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
) -> Result<Vec<usize>> {
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
    Ok(columns)
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
    let recovered = recover_ppml_fixed_effects(
        model,
        report,
        fit.linear().weights(),
        // These two independent inputs let the recovery bridge verify that
        // the explicit linear-frame weights are exactly q*mu.
        prior_weights,
        fit.mean(),
        fit.linear().absorbed_predictor(),
        fit.linear().fit().recovered_fe_coefficients(),
        lsmr,
        map,
        inject_allocation_failure,
    )?;
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
        strict_final_solve_passed: linear.fit().rank_certificate().passed
            && linear.fit().score_certificate().passed
            && linear.fit().bread().certificate().passed,
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

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn fallible_copy<T: Copy>(source: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| allocation(message))?;
    output.extend_from_slice(source);
    Ok(output)
}

fn fallible_filled<T: Copy>(length: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| allocation(message))?;
    output.resize(length, value);
    Ok(output)
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
mod tests {
    use super::*;
    use crate::data::{DropReason, WeightKind};
    use crate::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind,
        ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId,
        TermComponent, TermId, TermOrder, TimeSeriesState,
    };
    use crate::ols::{OlsInputBinding, OlsInputRole};
    use crate::ppml::prepared_irls::{
        reset_canonical_frame_refresh_count, take_canonical_frame_refresh_count,
        take_cmg_early_memory_peaks,
    };
    use crate::ppml::{PpmlFallbackKind, PPML_STEADY_STATE_AUDIT_ITERATIONS};
    use crate::ppml_separation::{MethodStatus, SeparationDropReason, SeparationMethods};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static FIT_PHASE_BEGINS: AtomicUsize = AtomicUsize::new(0);

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
        loaded.ols.candidate[11] = false;
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

    fn count_fit_phase_begins(
        _phase: PpmlPhase,
        event: super::super::PpmlPhaseAllocatorEvent,
    ) -> u64 {
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

        let error =
            fit_prepared_ppml(request, loaded).expect_err("one-FE PPML LSMR must fail closed");
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
                let selected = if effects >= 2 {
                    PpmlSolverFamily::Lsmr
                } else {
                    PpmlSolverFamily::Map
                };
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
    fn automatic_lsmr_admission_preserves_map_only_surfaces() {
        let (mut request, _) = fixture(2, false, false, false, false, false);
        assert!(auto_lsmr_eligible(&request));
        request.memory.requested = MemoryMode::Low;
        assert!(!auto_lsmr_eligible(&request));
        request.memory.requested = MemoryMode::Auto;
        request.allocation_snapshot_probe = Some(stable_allocation_probe);
        assert!(!auto_lsmr_eligible(&request));
        request.allocation_snapshot_probe = None;
        request.ols.absorbed_terms[0]
            .slope_sources
            .push(SourceId::new(1).expect("source"));
        assert!(!auto_lsmr_eligible(&request));
        request.ols.absorbed_terms[0].slope_sources.clear();
        request.ols.absorbed_terms[0].intercept = false;
        assert!(!auto_lsmr_eligible(&request));
        let (request, _) = fixture(4, false, false, false, false, false);
        assert!(!auto_lsmr_eligible(&request));
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
        let baseline = fit_prepared_ppml(baseline_request, baseline_loaded)
            .expect("production MAP PPML oracle");
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
        let baseline = fit_prepared_ppml(baseline_request, baseline_loaded)
            .expect("production MAP PPML oracle");
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
    fn automatic_workspace_budget_veto_selects_map_without_solver_retry() {
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
        assert_eq!(automatic.memory().plan.solver_workspace_bytes, 0);
        for engine in &automatic.solver().engines()[1..] {
            assert_eq!(engine.iterations, 0);
            assert_eq!(engine.retries, 0);
            assert_eq!(engine.setup_seconds, 0.0);
        }
        assert_ppml_results_close(&automatic, &explicit);
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
        let (topology_only, complete) =
            take_cmg_early_memory_peaks().expect("CMG early memory peaks");
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
        let baseline = fit_prepared_ppml(baseline_request, baseline_loaded)
            .expect("production MAP PPML oracle");
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
        assert_eq!(
            canonical_refreshes.saturating_add(1),
            map.checkpoint_frames + solver.engines()[1].checkpoint_frames,
            "every canonical checkpoint is attributed to its computing engine",
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
    fn explicit_public_map_matches_one_effect_auto_bitwise() {
        let (mut baseline_request, baseline_loaded) = fixture(1, false, false, false, false, false);
        baseline_request.solver = PpmlSolverFamily::Auto;
        let baseline = fit_prepared_ppml(baseline_request, baseline_loaded)
            .expect("production MAP PPML oracle");
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
            result.convergence().outer_iterations
                + result.convergence().strict_profile_rejections
                + 2
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
    fn final_sample_rebase_preserves_the_full_original_omission_axis() {
        let (request, loaded) = factor_rebase_fixture();
        let result = fit_prepared_ppml(request, loaded).expect("rebased factor composite PPML");
        assert_eq!(
            result.coefficients().statuses(),
            &[
                // The caller-declared base vanished during numeric sample
                // preparation but retains its canonical original identity.
                PpmlCoefficientStatus::BaseLevel,
                // The lowest surviving category becomes the numerical base;
                // it was not caller-declared base metadata.
                PpmlCoefficientStatus::SyntaxOmitted,
                PpmlCoefficientStatus::Identified,
                PpmlCoefficientStatus::SyntaxOmitted,
                PpmlCoefficientStatus::Identified,
            ]
        );
        assert_eq!(result.coefficients().kept_full_slots(), &[2, 4]);
        assert_eq!(result.coefficients().values().len(), 5);
        assert_eq!(result.inference().vce.covariance.len(), 25);
        for &slot in &[0_usize, 1, 3] {
            assert_eq!(result.coefficients().values()[slot], 0.0);
            assert!(result.inference().vce.standard_errors[slot].is_nan());
        }
        assert_eq!(
            result.memory().plan.dimensions.reported_coefficient_slots,
            5
        );
        assert!(result.memory().plan.dimensions.coefficient_slots < 5);
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
            certificate.measured_iterations,
            PPML_STEADY_STATE_AUDIT_ITERATIONS
        );
    }

    #[test]
    fn one_and_two_fe_ppml_map_keep_fused_projection_capacities_stable() {
        for effects in [1, 2] {
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
            assert!(certificate.linear_session_allocation_free);
            assert!(certificate.fresh_warm_equivalent);
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
                    generation.outcome(),
                    generation.prior_weights(),
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
        loaded.ols.candidate = vec![true; rows];
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
        let residual = fit_prepared_ppml(residual_request, residual_loaded)
            .expect("residual-only PPML result");
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
        assert!(result
            .output()
            .offset()
            .iter()
            .zip(exposure)
            .all(|(offset, exposure)| (*offset - exposure.ln()).abs() <= 2e-16));
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
                    let error =
                        result.expect_err("exhausted recovery must return no composite result");
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
            for (dimension, (_, column)) in
                loaded.ols.numeric_sources.iter_mut().enumerate().skip(1)
            {
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
                    let directions =
                        levels * (usize::from(term.intercept) + term.slope_sources.len());
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
        let error =
            refine_preflight_fe_bounds(&request.ols, &loaded.ols, &mut dimensions, &mut |_| {
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
        let exact = crate::factorize::interaction_codes(&[first.as_slice()])
            .expect("first")
            .n_levels()
            + crate::factorize::interaction_codes(&[second.as_slice()])
                .expect("second")
                .n_levels()
            + crate::factorize::interaction_codes(&[first.as_slice(), second.as_slice()])
                .expect("joint")
                .n_levels();
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
                stata_var_position: u32::try_from(numeric_sources.len() + 3)
                    .expect("weight position"),
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
            ols: LoadedOlsInput {
                candidate: vec![true; rows],
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
            ols: LoadedOlsInput {
                candidate: vec![true; rows],
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
}
