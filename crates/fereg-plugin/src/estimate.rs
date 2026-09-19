use crate::memory_report::{Baseline, Report as MemoryReport, ROWS as MEMORY_ROWS};
use std::collections::{BTreeMap, BTreeSet};
use crate::input_memory::{InputMemory, StringDictionary};
use std::time::Instant;

use crate::input::{PreparedMatrixStoreName, SpiApi};
use crate::instrumentation::{self, ResourceSnapshot};
use crate::result::{
    build_shadow_result, CertifiedOlsInference, OlsShadowResult, OlsVceKind, CERTIFICATE_ROWS,
    CLUSTER_COLUMNS, COEFFICIENT_STATUS_COLUMNS, DOF_ROWS, FE_COLUMNS, FIT_ROWS, RECEIPT_COLUMNS,
    RECEIPT_COMPLETE_COLUMN, RESULT_MAGIC, RUNTIME_ROWS, SOLVER_COLUMNS, SOLVER_ROWS,
    SUBSET_COLUMNS,
};
use crate::result_view::{PreparedOlsClusteredResultData, PreparedOlsResultData};
use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::fe::{
    plan_categorical_absorbed_request, AbsorbConfig, FeSolverKind, LsmrConfig, LsmrPreconditioner,
};
use fereg_core::metadata::{
    ComponentKind, ModelWireV2, NumericWireTable, SourceId, ABSORB_COMPONENT_ROW_WIDTH,
    ABSORB_TERM_ROW_WIDTH, COMPONENT_ROW_WIDTH, SOURCE_ROW_WIDTH, TERM_ROW_WIDTH,
};
use fereg_core::ols::{
    fit_prepared_categorical_ols_clustered_map, fit_prepared_categorical_ols_robust_map,
    fit_prepared_categorical_ols_unadjusted_map, fit_prepared_ols_clustered_map,
    fit_prepared_ols_robust_map, fit_prepared_ols_unadjusted_map,
    prepare_categorical_ols_clustered_data_owned, prepare_ols_clustered_data_owned,
    prepare_ols_data_owned, CertifiedPreparedOlsFit, LoadedNumericColumn, LoadedOlsInput,
    OlsClusterTermV1, OlsClusteredRequestV1, OlsInputBinding, OlsInputRole, OlsRequestV1,
    PreparedOlsAbsorbedDofPolicy, PreparedOlsClusteredConfig, PreparedOlsExecutionConfig,
    PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsFitStatus, PreparedOlsRobustConfig,
    PreparedOlsUnadjustedConfig,
};
use fereg_core::prepare_categorical_ols_data_owned;
use fereg_core::protocol::{
    parse_key_values, Handshake, ACTION_PROTOCOL_VERSION, OLS_RESULT_WIRE_VERSION, PLUGIN_VERSION,
};
use fereg_core::{
    estimate_phase2_memory, CategoricalTopologyDofReport, FeSolverMemoryBudget, MapConfig,
    MemoryMode, MemoryPlan, PreparedCategoricalOlsData, ThreadConfig, ThreadTeam,
};
use rayon::prelude::*;

pub const ACTION_ESTIMATE_OLS: &str = "estimate-ols";
const CONFIRMATION: &str = "fereg-estimation";
const BINDING_ROW_WIDTH: usize = 3;
const CLUSTER_TERM_ROW_WIDTH: usize = 2;
const CLUSTER_COMPONENT_ROW_WIDTH: usize = 3;

#[derive(Debug, Clone)]
struct ResultMatrices {
    b: String,
    covariance: String,
    coefficient_status: String,
    omission_b_marker: String,
    omission_v_marker: String,
    fit: String,
    dof: String,
    solver: String,
    fixed_effects: String,
    clusters: String,
    subsets: String,
    certificates: String,
    runtime: String,
    memory_report: String,
    receipt: String,
}

#[derive(Debug)]
struct PreparedResultMatrices {
    b: PreparedMatrixStoreName,
    covariance: PreparedMatrixStoreName,
    coefficient_status: PreparedMatrixStoreName,
    omission_b_marker: PreparedMatrixStoreName,
    omission_v_marker: PreparedMatrixStoreName,
    fit: PreparedMatrixStoreName,
    dof: PreparedMatrixStoreName,
    solver: PreparedMatrixStoreName,
    fixed_effects: PreparedMatrixStoreName,
    clusters: PreparedMatrixStoreName,
    subsets: PreparedMatrixStoreName,
    certificates: PreparedMatrixStoreName,
    runtime: PreparedMatrixStoreName,
    memory_report: PreparedMatrixStoreName,
    receipt: PreparedMatrixStoreName,
}

#[derive(Debug, Clone)]
struct ObservationOutputs {
    sample: u32,
    residual: Option<u32>,
    drop_reason: Option<u32>,
    xb: Option<u32>,
    xbd: Option<u32>,
    absorbed: Option<u32>,
    saved_channels: Vec<Option<u32>>,
}

#[derive(Debug)]
struct DerivedObservationOutputs {
    xb: Option<Vec<f64>>,
    xbd: Option<Vec<f64>>,
    absorbed: Option<Vec<f64>>,
    saved_channels: Vec<Option<Vec<f64>>>,
}

impl DerivedObservationOutputs {
    fn capacity_bytes(&self) -> u128 {
        let values = [&self.xb, &self.xbd, &self.absorbed]
            .into_iter()
            .chain(self.saved_channels.iter())
            .map(|value| value.as_ref().map_or(0, Vec::capacity) as u128)
            .sum::<u128>();
        values * std::mem::size_of::<f64>() as u128
            + self.saved_channels.capacity() as u128
                * std::mem::size_of::<Option<Vec<f64>>>() as u128
    }
}

#[derive(Debug)]
struct EstimateAction {
    memory_baseline: Baseline,
    transport_parent_bytes: u128,
    request: OlsRequestV1,
    candidate_all: bool,
    input_complete: bool,
    clusters: Vec<OlsClusterTermV1>,
    vce: OlsVceKind,
    absorbed_dof: PreparedOlsAbsorbedDofPolicy,
    account_cluster_nesting: bool,
    fit: PreparedOlsFitConfig,
    expected_effective_threads: usize,
    memory_plan: MemoryPlan,
    preparation_memory: crate::ols_preparation_memory::OlsPreparationMemory,
    frame_memory: crate::ols_frame_memory::OlsFrameMemory,
    generated_columns: usize,
    prepared_owner_bytes: u128,
    execution_parent_bytes: u128,
    output_parent_bytes: u128,
    metadata_owner_bytes: u128,
    metadata_peak_bytes: u128,
    input_peak_bytes: u128,
    publication_memory: crate::ols_publication_memory::OlsPublicationMemory,
    requested_tolerance: f64,
    outputs: ObservationOutputs,
    matrices: PreparedResultMatrices,
}

#[derive(Debug)]
struct EstimateActionSpec {
    memory_baseline: Baseline,
    transport_parent_bytes: u128,
    transport_peak_bytes: u128,
    model_major: f64,
    model_minor: f64,
    source_matrix: String,
    source_count: usize,
    term_matrix: String,
    term_count: usize,
    component_matrix: String,
    component_count: usize,
    source_names: Vec<String>,
    canonical_tokens: Vec<String>,
    raw_component_operators: Vec<String>,
    raw_time_series_operators: Vec<String>,
    absorb_term_matrix: String,
    absorb_term_count: usize,
    absorb_component_matrix: String,
    absorb_component_count: usize,
    absorb_labels: Vec<String>,
    absorb_saved_names: Vec<String>,
    cluster_term_matrix: String,
    cluster_term_count: usize,
    cluster_component_matrix: String,
    cluster_component_count: usize,
    cluster_labels: Vec<String>,
    binding_matrix: String,
    binding_count: usize,
    include_intercept: bool,
    candidate_all: bool,
    input_complete: bool,
    keep_singletons: bool,
    vce: OlsVceKind,
    absorbed_dof: PreparedOlsAbsorbedDofPolicy,
    account_cluster_nesting: bool,
    memory: MemoryMode,
    solver: FeSolverKind,
    preconditioner: LsmrPreconditioner,
    poolsize: usize,
    threads: usize,
    deterministic: bool,
    tolerance: f64,
    iterate: usize,
    outputs: ObservationOutputs,
    matrices: ResultMatrices,
}

#[derive(Debug)]
struct ActionObservation {
    started: Instant,
    resources_start: ResourceSnapshot,
    preflight_seconds: f64,
    input_load_seconds: f64,
}

// Stata interruption polling crosses the Rust/C/SPI boundary. Polling every
// row doubled that boundary traffic during input loading without improving
// rollback semantics. A bounded stride keeps Break latency short while the
// final poll below guarantees no estimation starts after an unobserved stop.
const INPUT_INTERRUPT_POLL_STRIDE: usize = 1_024;
const INPUT_BULK_READ_CHUNK_ROWS: usize = 65_536;
const INPUT_CANDIDATE_BATCH_MIN_ROWS: usize = 16_384;
const INPUT_BULK_MIN_RUN_ROWS: usize = 64;

pub fn estimate_ols_test(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {
    #[cfg(test)]
    let _action_observation_guard = instrumentation::lock_action_observation();
    let started = Instant::now();
    let resources_start = instrumentation::begin_observation();
    let local_bundle = read_local_estimate_bundle(args, spi)?.map(|value| [value]);
    let mut specification = EstimateActionSpec::parse(
        local_bundle.as_ref().map_or(args, |values| values.as_slice()),
    )?;
    // The expanded local bundle dies here; only the original ABI argv
    // remains live through numerical work and publication.
    specification.transport_parent_bytes = crate::metadata_memory::argument_parent_bytes(args)?;
    drop(local_bundle);
    let mut action = specification.decode_and_validate(spi)?;
    drop(specification);
    let preflight_seconds = started.elapsed().as_secs_f64();
    let mut input_memory = loading_memory(
        &action.request, spi.nobs()?, 0,
        action.metadata_owner_bytes, action.memory_plan.usable_bytes,
    )?;
    let input_team = (action.candidate_all
        && action.request.absorbed_terms.len() == 1
        && action.request.absorbed_terms[0].categorical_sources.len() == 1
        && action.request.absorbed_terms[0].slope_sources.is_empty()
        && action.request.absorbed_terms[0].saved_name.is_empty()
        && matches!(action.vce, OlsVceKind::Iid)
        && !action.request.bindings.iter().any(|binding| matches!(binding.role,
            OlsInputRole::AnalyticWeight | OlsInputRole::FrequencyWeight | OlsInputRole::ProbabilityWeight)))
        .then(|| ThreadTeam::new(action.fit.execution.threads));
    let loaded = load_ols_input_with_team(
        &action.request,
        action.candidate_all,
        action.input_complete,
        &mut input_memory,
        spi,
        input_team,
    )?;
    action.input_peak_bytes = input_memory.peak();
    let input_load_seconds = started.elapsed().as_secs_f64() - preflight_seconds;
    let observation = ActionObservation {
        started,
        resources_start,
        preflight_seconds,
        input_load_seconds,
    };
    let categorical = action.request.absorbed_terms.iter().any(|term| {
        term.categorical_sources.len() > 1
            || !term.slope_sources.is_empty()
            || !term.saved_name.is_empty()
    });
    match (action.vce, categorical) {
        (OlsVceKind::Iid, false) => {
            let trace_enabled = std::env::var_os("FEREG_PHASE_TIMING").is_some();
            let trace_started = Instant::now();
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_ols_data_owned(&action.request, loaded)
            })?;
            action.admit_prepared_parent(prepared.owned_buffer_bytes())?;
            let prepare_seconds = trace_started.elapsed().as_secs_f64();
            let inference_started = Instant::now();
            let inference = fit_prepared_ols_unadjusted_map(
                &prepared,
                PreparedOlsUnadjustedConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsUnadjustedConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let inference_seconds = inference_started.elapsed().as_secs_f64();
            let outputs_started = Instant::now();
            let result_data = PreparedOlsResultData::legacy(&prepared);
            let derived = build_derived_observation_outputs(
                result_data,
                None,
                inference.fit(),
                &action.outputs,
                false,
                action.fit,
            )?;
            let outputs_seconds = outputs_started.elapsed().as_secs_f64();
            let shadow_started = Instant::now();
            let shadow = build_shadow_result(
                result_data,
                None,
                CertifiedOlsInference::Iid(&inference),
                action.outputs.residual.is_some(),
            )?;
            let shadow_seconds = shadow_started.elapsed().as_secs_f64();
            if trace_enabled {
                eprintln!(
                    "FEREG_PHASE|iid_plugin|prepare={prepare_seconds:.9}|inference={inference_seconds:.9}|outputs={outputs_seconds:.9}|shadow={shadow_seconds:.9}|prestore_total={:.9}",
                    trace_started.elapsed().as_secs_f64()
                );
            }
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Robust, false) => {
            let trace_enabled = std::env::var_os("FEREG_PHASE_TIMING").is_some();
            let trace_started = Instant::now();
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_ols_data_owned(&action.request, loaded)
            })?;
            action.admit_prepared_parent(prepared.owned_buffer_bytes())?;
            let prepare_seconds = trace_started.elapsed().as_secs_f64();
            let inference_started = Instant::now();
            let inference = fit_prepared_ols_robust_map(
                &prepared,
                PreparedOlsRobustConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsRobustConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let inference_seconds = inference_started.elapsed().as_secs_f64();
            let outputs_started = Instant::now();
            let result_data = PreparedOlsResultData::legacy(&prepared);
            let derived = build_derived_observation_outputs(
                result_data,
                None,
                inference.fit(),
                &action.outputs,
                false,
                action.fit,
            )?;
            let outputs_seconds = outputs_started.elapsed().as_secs_f64();
            let shadow_started = Instant::now();
            let shadow = build_shadow_result(
                result_data,
                None,
                CertifiedOlsInference::Robust(&inference),
                action.outputs.residual.is_some(),
            )?;
            let shadow_seconds = shadow_started.elapsed().as_secs_f64();
            if trace_enabled {
                eprintln!(
                    "FEREG_PHASE|robust_plugin|prepare={prepare_seconds:.9}|inference={inference_seconds:.9}|outputs={outputs_seconds:.9}|shadow={shadow_seconds:.9}|prestore_total={:.9}",
                    trace_started.elapsed().as_secs_f64()
                );
            }
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Clustered, false) => {
            let request = OlsClusteredRequestV1 {
                ols: action.request.clone(),
                clusters: action.clusters.clone(),
            };
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_ols_clustered_data_owned(&request, loaded)
            })?;
            drop(request);
            action.admit_prepared_parent(prepared.ols().owned_buffer_bytes() + prepared.clusters().owned_buffer_bytes())?;
            let inference = fit_prepared_ols_clustered_map(
                &prepared,
                PreparedOlsClusteredConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    account_cluster_nesting: action.account_cluster_nesting,
                    ..PreparedOlsClusteredConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let result_data = PreparedOlsResultData::legacy(prepared.ols());
            let derived = build_derived_observation_outputs(
                result_data,
                None,
                inference.fit(),
                &action.outputs,
                false,
                action.fit,
            )?;
            let shadow = build_shadow_result(
                result_data,
                Some(PreparedOlsClusteredResultData::legacy(&prepared)),
                CertifiedOlsInference::Clustered(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Iid, true) => {
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_categorical_ols_data_owned(&action.request, loaded)
            })?;
            action.admit_prepared_parent(prepared.owned_buffer_bytes())?;
            let inference = fit_prepared_categorical_ols_unadjusted_map(
                &prepared,
                PreparedOlsUnadjustedConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsUnadjustedConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let result_data = PreparedOlsResultData::categorical(&prepared);
            let derived = build_derived_observation_outputs(
                result_data,
                Some(&prepared),
                inference.fit(),
                &action.outputs,
                normalization_known_connected(inference.absorbed_dof()),
                action.fit,
            )?;
            let shadow = build_shadow_result(
                result_data,
                None,
                CertifiedOlsInference::CategoricalIid(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Robust, true) => {
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_categorical_ols_data_owned(&action.request, loaded)
            })?;
            action.admit_prepared_parent(prepared.owned_buffer_bytes())?;
            let inference = fit_prepared_categorical_ols_robust_map(
                &prepared,
                PreparedOlsRobustConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsRobustConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let result_data = PreparedOlsResultData::categorical(&prepared);
            let derived = build_derived_observation_outputs(
                result_data,
                Some(&prepared),
                inference.fit(),
                &action.outputs,
                normalization_known_connected(inference.absorbed_dof()),
                action.fit,
            )?;
            let shadow = build_shadow_result(
                result_data,
                None,
                CertifiedOlsInference::CategoricalRobust(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Clustered, true) => {
            let request = OlsClusteredRequestV1 {
                ols: action.request.clone(),
                clusters: action.clusters.clone(),
            };
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_categorical_ols_clustered_data_owned(&request, loaded)
            })?;
            drop(request);
            action.admit_prepared_parent(prepared.ols().owned_buffer_bytes() + prepared.clusters().owned_buffer_bytes())?;
            let inference = fit_prepared_categorical_ols_clustered_map(
                &prepared,
                PreparedOlsClusteredConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    account_cluster_nesting: action.account_cluster_nesting,
                    ..PreparedOlsClusteredConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let result_data = PreparedOlsResultData::categorical(prepared.ols());
            let derived = build_derived_observation_outputs(
                result_data,
                Some(prepared.ols()),
                inference.fit(),
                &action.outputs,
                normalization_known_connected(inference.absorbed_dof()),
                action.fit,
            )?;
            let shadow = build_shadow_result(
                result_data,
                Some(PreparedOlsClusteredResultData::categorical(&prepared)),
                CertifiedOlsInference::CategoricalClustered(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
    }
}

// Loading has finished before this boundary; the closure captures no SPI session.
// Reuse the solve's bounded team for wide native classification and compaction.
fn prepare_loaded_ols<T: Send>(
    loaded: LoadedOlsInput,
    threads: ThreadConfig,
    prepare: impl FnOnce(LoadedOlsInput) -> Result<T> + Send,
) -> Result<T> {
    let started = Instant::now();
    let prepared = if loaded.numeric_sources.len() >= 16 && loaded.candidate.len() >= 16_384 {
        ThreadTeam::new(threads).install(|| prepare(loaded))??
    } else {
        prepare(loaded)?
    };
    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!(
            "FEREG_PHASE|ols_prepare|seconds={:.9}",
            started.elapsed().as_secs_f64()
        );
    }
    Ok(prepared)
}

impl EstimateAction {
    /// Translate existing component models to one explicit action timeline.
    /// This keeps unproven owner families visible instead of treating a
    /// solver-only report as a complete command-memory estimate.
    fn memory_ledger(&self, solver: Option<u64>) -> Result<crate::action_memory::Ledger> {
        use crate::action_memory::{Estimator, Frame, Gaps, Ledger, Stage};
        let mut ledger = Ledger::new(Estimator::Ols);
        ledger.record(
            Stage::Metadata,
            Frame {
                parents: 0,
                workspace: self.metadata_peak_bytes,
            },
            Gaps::TRANSPORT_COMPILER,
        )?;
        ledger.record(
            Stage::Loading,
            Frame {
                parents: 0,
                workspace: self.input_peak_bytes,
            },
            Gaps::TRANSPORT_COMPILER.union(Gaps::WORKER_RUNTIME),
        )?;
        ledger.record(
            Stage::Preparation,
            Frame {
                parents: self.metadata_owner_bytes,
                workspace: self.preparation_memory.preparation_peak_bytes(0)?,
            },
            Gaps::CONSTRUCTION.union(Gaps::WORKER_RUNTIME),
        )?;
        let mut fit_gaps = Gaps::NUMERICAL_FRAMES
            .union(Gaps::RETAINED_FE_PARENT)
            .union(Gaps::WORKER_RUNTIME);
        if !self.request.absorbed_terms.is_empty() {
            fit_gaps = fit_gaps.union(Gaps::DEPENDENCY_CONTAINERS);
        }
        if solver.is_none() {
            fit_gaps = fit_gaps.union(Gaps::SOLVER_ATTEMPT);
        }
        // Inference projections are included in the fit's additional solver
        // peak. Use the same conservative frame in both sequential stages.
        let fit = Frame {
            parents: self.execution_parent_bytes,
            workspace: u128::from(solver.unwrap_or(0)),
        };
        ledger.record(Stage::Fit, fit, fit_gaps)?;
        ledger.record(Stage::Inference, fit, fit_gaps)?;
        let recovery = self.outputs.saved_channels.iter().any(Option::is_some);
        ledger.record(
            Stage::Recovery,
            Frame {
                parents: self.output_parent_bytes,
                workspace: self.publication_memory.derived_peak_bytes,
            },
            if recovery {
                Gaps::RECOVERY
                    .union(Gaps::WORKER_RUNTIME)
                    .union(Gaps::DEPENDENCY_CONTAINERS)
            } else {
                Gaps::WORKER_RUNTIME
            },
        )?;
        ledger.record(
            Stage::Publication,
            Frame {
                parents: self.output_parent_bytes,
                workspace: self.publication_memory.peak_bytes(),
            },
            Gaps::WORKER_RUNTIME,
        )?;
        Ok(ledger)
    }

    /// Expected lifetime composition; conservative admission remains unchanged.
    /// A fast projection owns one RHS bank. Finite/inference banks are later
    /// sequential frames and must not all become the projection's live parent.
    fn memory_forecast(&self, shadow: &OlsShadowResult<'_>) -> Result<Option<crate::memory_forecast::Forecast>> {
        use crate::memory_forecast::{sum, Forecast, ols_projection_parent};
        let Some(solver) = shadow.solver_expected_heap_peak else { return Ok(None); };
        let fast = shadow.execution.mode == PreparedOlsExecutionMode::Fast;
        let fit = if fast {
            let projection = sum(&[ols_projection_parent(self.prepared_owner_bytes,
                self.metadata_owner_bytes, shadow.retained_rows.len(), self.generated_columns,
                self.fit.execution.poolsize, true)?, u128::from(solver)])?;
            let finite = sum(&[self.prepared_owner_bytes, self.metadata_owner_bytes,
                self.preparation_memory.frame_metadata_bytes, self.frame_memory.fast_bytes])?;
            projection.max(finite)
        } else {
            // Low-memory score reconstruction has different retained parents;
            // retain its conservative composition until that case is calibrated.
            sum(&[self.execution_parent_bytes, u128::from(solver)])?
        };
        let preparation = sum(&[self.metadata_owner_bytes,
            self.preparation_memory.preparation_peak_bytes(0)?])?;
        let publication = sum(&[self.output_parent_bytes, self.publication_memory.peak_bytes()])?;
        let native_peak = fit.max(preparation).max(publication)
            .max(self.metadata_peak_bytes).max(self.input_peak_bytes);
        let matrix_cells = self.publication_memory.shadow_bytes / 8
            + (crate::result::RUNTIME_ROWS * 2 + crate::memory_report::CELLS) as u128;
        Ok(Some(Forecast { native_peak,
            stata_growth: Forecast::publication_growth(shadow.drop_reasons.len(), matrix_cells,
                crate::memory_forecast::has_factor_stripes(&self.request.model),
                self.memory_baseline.before_allocated)? }))
    }

    fn metadata_capacity_bytes(&self) -> u128 {
        use std::mem::size_of;
        let matrices = &self.matrices;
        self.request.owned_buffer_bytes()
            + self.clusters.capacity() as u128 * size_of::<OlsClusterTermV1>() as u128
            + self.clusters.iter().map(|cluster| {
                cluster.label.capacity() as u128
                    + cluster.sources.capacity() as u128 * size_of::<SourceId>() as u128
            }).sum::<u128>()
            + self.outputs.saved_channels.capacity() as u128 * size_of::<Option<u32>>() as u128
            // CString owns a boxed byte slice, including its NUL terminator.
            + [&matrices.b, &matrices.covariance, &matrices.coefficient_status,
                &matrices.omission_b_marker, &matrices.omission_v_marker, &matrices.fit,
                &matrices.dof, &matrices.solver, &matrices.fixed_effects, &matrices.clusters,
                &matrices.subsets, &matrices.certificates, &matrices.runtime, &matrices.memory_report, &matrices.receipt]
                .iter().map(|name| name.as_c_str().to_bytes_with_nul().len() as u128).sum::<u128>()
    }

    /// Reconcile the surviving preparation owner before fit/inference allocates
    /// its frames. Public base estimates retain their historical meaning.
    fn admit_prepared_parent(&mut self, owned_bytes: u128) -> Result<()> {
        let public_base = self.memory_plan.estimate;
        let reconciled = self.preparation_memory.with_prepared_owner(public_base, owned_bytes)?;
        let frame_parent = owned_bytes.checked_add(self.preparation_memory.frame_metadata_bytes)
            .ok_or_else(|| allocation("OLS transient metadata parent overflows"))?;
        let mut execution = self.frame_memory.with_parent(reconciled, frame_parent)?;
        execution.fast_bytes = execution.fast_bytes.checked_add(self.metadata_owner_bytes)
            .ok_or_else(|| allocation("OLS execution metadata size overflows"))?;
        execution.low_bytes = execution.low_bytes.checked_add(self.metadata_owner_bytes)
            .ok_or_else(|| allocation("OLS execution metadata size overflows"))?;
        let mut plan = self.publication_memory.plan(
            self.memory_plan.requested,
            execution,
            self.memory_plan.available_bytes,
        )?;
        let execution_bytes = plan.selected_bytes();
        self.fit.execution.mode = match plan.selected {
            MemoryMode::Fast => PreparedOlsExecutionMode::Fast,
            MemoryMode::Low => PreparedOlsExecutionMode::Low,
            MemoryMode::Auto => return Err(invalid("unresolved prepared OLS memory mode")),
        };
        self.fit.absorb.solver_memory_budget = plan.usable_bytes.map(|usable_bytes| {
            FeSolverMemoryBudget { execution_bytes, usable_bytes }
        });
        self.fit.absorb.retain_coefficients = self.outputs.saved_channels.iter().any(Option::is_some)
            && plan.selected == MemoryMode::Fast;
        plan.estimate = public_base;
        self.memory_plan = plan;
        self.prepared_owner_bytes = owned_bytes;
        self.execution_parent_bytes = execution_bytes;
        Ok(())
    }

    fn admit_output_parent(&mut self, inference_bytes: u128) -> Result<()> {
        let owned = self.prepared_owner_bytes.checked_add(inference_bytes)
            .and_then(|bytes| bytes.checked_add(self.metadata_owner_bytes))
            .ok_or_else(|| allocation("OLS output parent size overflows"))?;
        self.output_parent_bytes = owned;
        if let Some(budget) = self.fit.absorb.solver_memory_budget.as_mut() {
            let parent = budget.execution_bytes.max(owned);
            if !parent.checked_add(self.publication_memory.peak_bytes())
                .is_some_and(|peak| peak <= budget.usable_bytes) {
                return Err(allocation("OLS retained inference and publication exceed usable memory"));
            }
            budget.execution_bytes = parent;
        }
        Ok(())
    }
}

impl EstimateActionSpec {
    fn metadata_shape(&self) -> crate::metadata_memory::MetadataShape {
        let registry_bytes = [
            &self.source_names,
            &self.canonical_tokens,
            &self.raw_component_operators,
            &self.raw_time_series_operators,
            &self.absorb_labels,
            &self.absorb_saved_names,
            &self.cluster_labels,
        ].into_iter().map(crate::metadata_memory::strings_bytes).sum::<u128>();
        let string_bytes = [
            &self.source_matrix,
            &self.term_matrix,
            &self.component_matrix,
            &self.absorb_term_matrix,
            &self.absorb_component_matrix,
            &self.cluster_term_matrix,
            &self.cluster_component_matrix,
            &self.binding_matrix,
            &self.matrices.b,
            &self.matrices.covariance,
            &self.matrices.coefficient_status,
            &self.matrices.fit,
            &self.matrices.dof,
            &self.matrices.solver,
            &self.matrices.fixed_effects,
            &self.matrices.clusters,
            &self.matrices.subsets,
            &self.matrices.certificates,
            &self.matrices.runtime,
            &self.matrices.memory_report,
            &self.matrices.receipt,
            &self.matrices.omission_b_marker,
            &self.matrices.omission_v_marker,
        ].into_iter().map(|value| value.capacity() as u128).sum::<u128>();
        crate::metadata_memory::MetadataShape {
            rows: [self.source_count, self.term_count, self.component_count,
                self.absorb_term_count, self.absorb_component_count, self.cluster_term_count,
                self.cluster_component_count, self.binding_count],
            transport_parent_bytes: self.transport_parent_bytes,
            specification_bytes: registry_bytes + string_bytes
                + self.outputs.saved_channels.capacity() as u128 * std::mem::size_of::<Option<u32>>() as u128,
        }
    }

    fn parse(args: &[String]) -> Result<Self> {
        let transport_peak_bytes = admit_estimate_transport(args)?;
        let values = parse_estimate_key_values(args)?;
        let handshake = Handshake::from_key_values(&values)?;
        handshake.validate()?;
        if handshake.protocol != ACTION_PROTOCOL_VERSION {
            return Err(protocol(format!(
                "{ACTION_ESTIMATE_OLS} requires action protocol {}.{}",
                ACTION_PROTOCOL_VERSION.major, ACTION_PROTOCOL_VERSION.minor,
            )));
        }
        if required(&values, "confirm")? != CONFIRMATION {
            return Err(unsupported(format!(
                "{ACTION_ESTIMATE_OLS} requires confirm={CONFIRMATION}",
            )));
        }
        if required(&values, "plugin")? != PLUGIN_VERSION {
            return Err(protocol(format!(
                "requested plugin {} does not match loaded plugin {PLUGIN_VERSION}",
                required(&values, "plugin")?,
            )));
        }
        let result_major = parse_u16(&values, "result_major")?;
        let result_minor = parse_u16(&values, "result_minor")?;
        if result_major != OLS_RESULT_WIRE_VERSION.major
            || result_minor != OLS_RESULT_WIRE_VERSION.minor
        {
            return Err(protocol(format!(
                "OLS result wire {result_major}.{result_minor} is incompatible with {}.{}",
                OLS_RESULT_WIRE_VERSION.major, OLS_RESULT_WIRE_VERSION.minor,
            )));
        }
        let source_count = parse_usize(&values, "source_count")?;
        let term_count = parse_usize(&values, "term_count")?;
        let component_count = parse_usize(&values, "component_count")?;
        let absorb_term_count = parse_usize(&values, "absorb_term_count")?;
        let absorb_component_count = parse_usize(&values, "absorb_component_count")?;
        let cluster_term_count = parse_usize(&values, "cluster_term_count")?;
        let cluster_component_count = parse_usize(&values, "cluster_component_count")?;
        let output_residual = parse_nonnegative_u32(&values, "output_residual")?;
        let output_drop_reason = parse_nonnegative_u32(&values, "output_drop_reason")?;
        let output_xb = parse_nonnegative_u32(&values, "output_xb")?;
        let output_xbd = parse_nonnegative_u32(&values, "output_xbd")?;
        let output_absorbed = parse_nonnegative_u32(&values, "output_absorbed")?;
        let output_saved_count = parse_usize(&values, "output_saved_count")?;
        let specification = Self {
            memory_baseline: Baseline::parse(&values)?,
            transport_parent_bytes: crate::metadata_memory::argument_parent_bytes(args)?,
            transport_peak_bytes,
            model_major: f64::from(parse_u16(&values, "model_major")?),
            model_minor: f64::from(parse_u16(&values, "model_minor")?),
            source_matrix: required(&values, "source_matrix")?.to_owned(),
            source_count,
            term_matrix: required(&values, "term_matrix")?.to_owned(),
            term_count,
            component_matrix: required(&values, "component_matrix")?.to_owned(),
            component_count,
            source_names: decode_registry(required(&values, "source_names")?, source_count)?,
            canonical_tokens: decode_registry(required(&values, "canonical_tokens")?, term_count)?,
            raw_component_operators: decode_registry(
                required(&values, "raw_component_operators")?,
                component_count,
            )?,
            raw_time_series_operators: decode_registry(
                required(&values, "raw_time_series_operators")?,
                component_count,
            )?,
            absorb_term_matrix: required(&values, "absorb_term_matrix")?.to_owned(),
            absorb_term_count,
            absorb_component_matrix: required(&values, "absorb_component_matrix")?.to_owned(),
            absorb_component_count,
            absorb_labels: decode_registry(required(&values, "absorb_labels")?, absorb_term_count)?,
            absorb_saved_names: decode_registry(
                required(&values, "absorb_saved_names")?,
                absorb_term_count,
            )?,
            cluster_term_matrix: required(&values, "cluster_term_matrix")?.to_owned(),
            cluster_term_count,
            cluster_component_matrix: required(&values, "cluster_component_matrix")?.to_owned(),
            cluster_component_count,
            cluster_labels: decode_registry(
                required(&values, "cluster_labels")?,
                cluster_term_count,
            )?,
            binding_matrix: required(&values, "binding_matrix")?.to_owned(),
            binding_count: parse_usize(&values, "binding_count")?,
            include_intercept: parse_bool(&values, "include_intercept")?,
            candidate_all: parse_bool(&values, "candidate_all")?,
            input_complete: parse_bool(&values, "input_complete")?,
            keep_singletons: parse_bool(&values, "keep_singletons")?,
            vce: parse_vce(required(&values, "vce")?)?,
            absorbed_dof: parse_dof(required(&values, "dof")?)?,
            account_cluster_nesting: parse_bool(&values, "account_cluster_nesting")?,
            memory: MemoryMode::parse(required(&values, "memory")?)?,
            solver: parse_solver(required(&values, "solver")?)?,
            preconditioner: parse_preconditioner(required(&values, "preconditioner")?)?,
            poolsize: parse_positive_usize(&values, "poolsize")?,
            threads: parse_usize(&values, "threads")?,
            deterministic: parse_bool(&values, "deterministic")?,
            tolerance: parse_positive_f64(&values, "tolerance")?,
            iterate: parse_positive_usize(&values, "iterate")?,
            outputs: ObservationOutputs {
                sample: parse_positive_u32(&values, "output_sample")?,
                residual: (output_residual != 0).then_some(output_residual),
                drop_reason: (output_drop_reason != 0).then_some(output_drop_reason),
                xb: (output_xb != 0).then_some(output_xb),
                xbd: (output_xbd != 0).then_some(output_xbd),
                absorbed: (output_absorbed != 0).then_some(output_absorbed),
                saved_channels: decode_output_position_registry(
                    required(&values, "output_saved_channels")?,
                    output_saved_count,
                )?,
            },
            matrices: ResultMatrices {
                b: required(&values, "output_b_matrix")?.to_owned(),
                covariance: required(&values, "output_v_matrix")?.to_owned(),
                coefficient_status: required(&values, "output_status_matrix")?.to_owned(),
                omission_b_marker: required(&values, "output_marker_b_matrix")?.to_owned(),
                omission_v_marker: required(&values, "output_marker_v_matrix")?.to_owned(),
                fit: required(&values, "output_fit_matrix")?.to_owned(),
                dof: required(&values, "output_dof_matrix")?.to_owned(),
                solver: required(&values, "output_solver_matrix")?.to_owned(),
                fixed_effects: required(&values, "output_fe_matrix")?.to_owned(),
                clusters: required(&values, "output_cluster_matrix")?.to_owned(),
                subsets: required(&values, "output_subset_matrix")?.to_owned(),
                certificates: required(&values, "output_certificate_matrix")?.to_owned(),
                runtime: required(&values, "output_runtime_matrix")?.to_owned(),
                memory_report: required(&values, "output_memory_matrix")?.to_owned(),
                receipt: required(&values, "output_receipt_matrix")?.to_owned(),
            },
        };
        specification.reject_unknown_fields(&values)?;
        Ok(specification)
    }

    fn reject_unknown_fields(&self, values: &BTreeMap<String, String>) -> Result<()> {
        const ALLOWED: &[&str] = &[
            "ado",
            "protocol_major",
            "protocol_minor",
            "stata",
            "confirm",
            "plugin",
            "result_major",
            "result_minor",
            "model_major",
            "model_minor",
            "source_matrix",
            "source_count",
            "term_matrix",
            "term_count",
            "component_matrix",
            "component_count",
            "source_names",
            "canonical_tokens",
            "raw_component_operators",
            "raw_time_series_operators",
            "absorb_term_matrix",
            "absorb_term_count",
            "absorb_component_matrix",
            "absorb_component_count",
            "absorb_labels",
            "absorb_saved_names",
            "cluster_term_matrix",
            "cluster_term_count",
            "cluster_component_matrix",
            "cluster_component_count",
            "cluster_labels",
            "binding_matrix",
            "binding_count",
            "include_intercept",
            "candidate_all",
            "input_complete",
            "keep_singletons",
            "vce",
            "dof",
            "account_cluster_nesting",
            "memory",
            "solver",
            "preconditioner",
            "poolsize",
            "threads",
            "deterministic",
            "tolerance",
            "iterate",
            "output_sample",
            "output_residual",
            "output_drop_reason",
            "output_xb",
            "output_xbd",
            "output_absorbed",
            "output_saved_count",
            "output_saved_channels",
            "output_b_matrix",
            "output_v_matrix",
            "output_status_matrix",
            "output_marker_b_matrix",
            "output_marker_v_matrix",
            "output_fit_matrix",
            "output_dof_matrix",
            "output_solver_matrix",
            "output_fe_matrix",
            "output_cluster_matrix",
            "output_subset_matrix",
            "output_certificate_matrix",
            "output_runtime_matrix",
            "output_memory_matrix",
            "stata_used_entry",
            "stata_alloc_entry",
            "stata_used_before",
            "stata_alloc_before",
            "output_receipt_matrix",
        ];
        for key in values.keys() {
            if !ALLOWED.contains(&key.as_str()) {
                return Err(syntax(
                    format!("unknown {ACTION_ESTIMATE_OLS} field {key}",),
                ));
            }
        }
        Ok(())
    }

    fn decode_and_validate(&self, spi: &mut dyn SpiApi) -> Result<EstimateAction> {
        self.decode_with_memory_limit(spi, imposed_memory_limit_bytes)
    }

    fn decode_with_memory_limit(
        &self,
        spi: &mut dyn SpiApi,
        available_bytes: impl FnOnce() -> Result<Option<u128>>,
    ) -> Result<EstimateAction> {
        if !matches!(self.solver, FeSolverKind::Auto | FeSolverKind::Lsmr)
            && self.preconditioner != LsmrPreconditioner::Off
        {
            return Err(syntax(
                "preconditioner() is valid only with solver(auto) or solver(lsmr)",
            ));
        }
        let available_bytes = available_bytes()?;
        let usable_bytes = fereg_core::memory::usable_memory_bytes(available_bytes);
        let metadata_peak_bytes = self.metadata_shape().admit(usable_bytes)?.max(self.transport_peak_bytes);
        crate::metadata_memory::admit(metadata_peak_bytes, usable_bytes)?;
        let wire = ModelWireV2 {
            major: self.model_major,
            minor: self.model_minor,
            sources: read_table(
                spi,
                &self.source_matrix,
                self.source_count,
                SOURCE_ROW_WIDTH,
                "source",
            )?,
            terms: read_table(
                spi,
                &self.term_matrix,
                self.term_count,
                TERM_ROW_WIDTH,
                "term",
            )?,
            components: read_table(
                spi,
                &self.component_matrix,
                self.component_count,
                COMPONENT_ROW_WIDTH,
                "component",
            )?,
            source_names: self.source_names.clone(),
            canonical_tokens: self.canonical_tokens.clone(),
            raw_component_operators: self.raw_component_operators.clone(),
            raw_time_series_operators: self.raw_time_series_operators.clone(),
            absorb_terms: read_table(
                spi,
                &self.absorb_term_matrix,
                self.absorb_term_count,
                ABSORB_TERM_ROW_WIDTH,
                "absorbed term",
            )?,
            absorb_components: read_table(
                spi,
                &self.absorb_component_matrix,
                self.absorb_component_count,
                ABSORB_COMPONENT_ROW_WIDTH,
                "absorbed component",
            )?,
            absorb_labels: self.absorb_labels.clone(),
            absorb_saved_names: self.absorb_saved_names.clone(),
        };
        let decoded = wire.decode_specification()?;
        drop(wire);
        let bindings = read_bindings(spi, &self.binding_matrix, self.binding_count)?;
        let request = OlsRequestV1 {
            model: decoded.model,
            absorbed_terms: decoded.absorbed_terms,
            bindings,
            include_intercept: self.include_intercept,
            keep_singletons: self.keep_singletons,
        };
        let clusters = read_clusters(self, spi)?;
        validate_vce(self.vce, &clusters, self.account_cluster_nesting)?;
        let categorical = request.absorbed_terms.iter().any(|term| {
            term.categorical_sources.len() > 1
                || !term.slope_sources.is_empty()
                || !term.saved_name.is_empty()
        });
        let prepared_model = if categorical {
            // Validate the original tuple identities and guarded categorical
            // intercept/one-slope contract before any observation read. The
            // availability request preserves the ordinary model and
            // coefficient layout while replacing each tuple with one
            // synthetic single-source availability marker, which is the
            // only representation accepted by the legacy compiler.
            let plan = plan_categorical_absorbed_request(&request)?;
            if self.vce == OlsVceKind::Clustered {
                let requested_clustered = OlsClusteredRequestV1 {
                    ols: request.clone(),
                    clusters: clusters.clone(),
                };
                validate_cluster_request_shape(&requested_clustered)?;
                OlsClusteredRequestV1 {
                    ols: plan.availability_request().clone(),
                    clusters: clusters.clone(),
                }
                .validate_and_compile()?
            } else {
                plan.availability_request().validate_and_compile()?
            }
        } else if self.vce == OlsVceKind::Clustered {
            let clustered_request = OlsClusteredRequestV1 {
                ols: request.clone(),
                clusters: clusters.clone(),
            };
            // This validates cluster-only sources without reading observations.
            validate_cluster_request_shape(&clustered_request)?;
            clustered_request.validate_and_compile()?
        } else {
            request.validate_and_compile()?
        };
        let extended_fe_rows = request
            .absorbed_terms
            .iter()
            .try_fold(0_usize, |total, term| {
                total
                    .checked_add(usize::from(term.intercept) + term.slope_sources.len())
                    .ok_or_else(|| allocation("extended absorbed-effect row count overflows"))
            })?;
        let loaded_numeric_columns = request
            .model
            .sources
            .len()
            .checked_add(1)
            .ok_or_else(|| allocation("loaded numeric-column count overflows"))?;
        let memory_estimate = estimate_phase2_memory(
            spi.nobs()?,
            loaded_numeric_columns,
            prepared_model.columns.len(),
            extended_fe_rows,
            clusters.len(),
            self.poolsize,
        )?;
        let publication_memory = crate::ols_publication_memory::OlsPublicationMemory::estimate(
            crate::ols_publication_memory::OlsPublicationShape {
                observations: spi.nobs()?,
                reported_coefficients: prepared_model.coefficients.len(),
                fe_channels: extended_fe_rows,
                cluster_dimensions: clusters.len(),
                xb: self.outputs.xb.is_some(),
                xbd: self.outputs.xbd.is_some(),
                absorbed: self.outputs.absorbed.is_some(),
                saved_channels: self
                    .outputs
                    .saved_channels
                    .iter()
                    .filter(|value| value.is_some())
                    .count(),
            },
        )?;
        let memory_plan =
            publication_memory.plan(self.memory, memory_estimate, available_bytes)?;
        let preparation_memory = crate::ols_preparation_memory::OlsPreparationMemory::estimate(
            spi.nobs()?,
            loaded_numeric_columns,
            request.bindings.iter().any(|binding| matches!(binding.role,
                OlsInputRole::AnalyticWeight | OlsInputRole::FrequencyWeight | OlsInputRole::ProbabilityWeight)),
            extended_fe_rows,
            clusters.len(),
        )?;
        let preparation_memory = preparation_memory.with_construction(spi.nobs()?, &request, &clusters)?;
        preparation_memory.admit_numeric_arrays(memory_plan.usable_bytes)?;
        let frame_memory = crate::ols_frame_memory::OlsFrameMemory::estimate(
            spi.nobs()?, prepared_model.columns.len(), prepared_model.coefficients.len(),
            clusters.len(), self.poolsize, self.vce != OlsVceKind::Iid,
        )?;
        let default_fit = PreparedOlsFitConfig::default();
        let thread_config = ThreadConfig::from_requested(self.threads, self.deterministic)?;
        let fit = PreparedOlsFitConfig {
            allow_final_fe_recovery: true,
            // Residual-derived outputs need pointwise accuracy even when low
            // memory deliberately omits retained FE coefficient products.
            final_residual_tolerance: (self.outputs.residual.is_some()
                || self.outputs.absorbed.is_some()
                || self.outputs.xbd.is_some()
                || self.outputs.saved_channels.iter().any(Option::is_some))
            .then_some(1e-14),
            absorb: AbsorbConfig {
                solver: self.solver,
                map: MapConfig {
                    tolerance: self.tolerance,
                    max_iterations: self.iterate,
                    ..default_fit.absorb.map
                },
                lsmr: LsmrConfig {
                    tolerance: self.tolerance,
                    max_iterations: self.iterate,
                    preconditioner: self.preconditioner,
                    threads: thread_config,
                    ..default_fit.absorb.lsmr
                },
                solver_memory_budget: memory_plan.usable_bytes.map(|usable_bytes| {
                    FeSolverMemoryBudget {
                        execution_bytes: memory_plan.selected_bytes(),
                        usable_bytes,
                    }
                }),
                retain_coefficients: self.outputs.saved_channels.iter().any(Option::is_some)
                    && memory_plan.selected == MemoryMode::Fast,
                ..default_fit.absorb
            },
            execution: PreparedOlsExecutionConfig {
                mode: match memory_plan.selected {
                    MemoryMode::Fast => PreparedOlsExecutionMode::Fast,
                    MemoryMode::Low => PreparedOlsExecutionMode::Low,
                    MemoryMode::Auto => {
                        return Err(invalid(
                            "memory preflight returned unresolved automatic mode",
                        ));
                    }
                },
                poolsize: self.poolsize,
                threads: thread_config,
            },
            ..default_fit
        };
        if self.outputs.saved_channels.len() != extended_fe_rows {
            return Err(invalid(
                "saved fixed-effect output layout differs from extended absorbed channels",
            ));
        }
        let mut saved_cursor = 0_usize;
        for term in &request.absorbed_terms {
            let expected = !term.saved_name.is_empty();
            let channels = usize::from(term.intercept) + term.slope_sources.len();
            for _ in 0..channels {
                if self.outputs.saved_channels[saved_cursor].is_some() != expected {
                    return Err(invalid(
                        "saved fixed-effect positions disagree with absorbed saved-name metadata",
                    ));
                }
                saved_cursor += 1;
            }
        }
        let matrices = validate_layout(
            &request,
            &self.outputs,
            &self.matrices,
            prepared_model.coefficients.len(),
            extended_fe_rows.max(1),
            self.cluster_term_count,
            spi,
            &[
                &self.source_matrix,
                &self.term_matrix,
                &self.component_matrix,
                &self.absorb_term_matrix,
                &self.absorb_component_matrix,
                &self.cluster_term_matrix,
                &self.cluster_component_matrix,
                &self.binding_matrix,
            ],
        )?;
        let mut action = EstimateAction {
            memory_baseline: self.memory_baseline,
            transport_parent_bytes: self.transport_parent_bytes,
            request,
            candidate_all: self.candidate_all,
            input_complete: self.input_complete,
            clusters,
            vce: self.vce,
            absorbed_dof: self.absorbed_dof,
            account_cluster_nesting: self.account_cluster_nesting,
            expected_effective_threads: fit.execution.threads.resolved(),
            fit,
            memory_plan,
            preparation_memory,
            frame_memory,
            generated_columns: prepared_model.columns.len(),
            prepared_owner_bytes: 0,
            execution_parent_bytes: 0,
            output_parent_bytes: 0,
            metadata_owner_bytes: 0,
            metadata_peak_bytes,
            input_peak_bytes: 0,
            publication_memory,
            requested_tolerance: self.tolerance,
            outputs: self.outputs.clone(),
            matrices,
        };
        action.metadata_owner_bytes = action.metadata_capacity_bytes()
            .checked_add(action.transport_parent_bytes)
            .ok_or_else(|| allocation("metadata parent size overflows"))?;
        action.preparation_memory.admit_numeric_arrays_with_parent(
            action.memory_plan.usable_bytes, action.metadata_owner_bytes,
        )?;
        action.admit_prepared_parent(0)?;
        Ok(action)
    }
}

// This is a metadata transport capacity, independent of N and finite rank.
// Refuse impossible descriptors before reserving or accessing the caller macro.
const MAX_LOCAL_ESTIMATE_BUNDLE_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn admit_estimate_transport(args: &[String]) -> Result<u128> {
    let bytes = crate::metadata_memory::transport_peak_bytes(args)?;
    crate::metadata_memory::admit(
        bytes,
        fereg_core::memory::usable_memory_bytes(imposed_memory_limit_bytes()?),
    )?;
    Ok(bytes)
}

pub(crate) fn read_local_estimate_bundle(
    args: &[String],
    spi: &mut dyn SpiApi,
) -> Result<Option<String>> {
    let Some(descriptor) = args
        .iter()
        .find_map(|argument| argument.strip_prefix("bundle_local="))
    else {
        return Ok(None);
    };
    if args.len() != 1 {
        return Err(syntax(
            "local estimation bundle must be the only action argument",
        ));
    }
    if descriptor.is_empty()
        || descriptor.starts_with('0')
        || !descriptor.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(syntax(
            "local estimation bundle byte count is not canonical",
        ));
    }
    let length = descriptor
        .parse::<usize>()
        .map_err(|_| allocation("local estimation bundle byte count overflows"))?;
    if length > MAX_LOCAL_ESTIMATE_BUNDLE_BYTES {
        return Err(allocation(
            "local estimation bundle exceeds the 16 MiB metadata capacity",
        ));
    }
    const PREFIX: &[u8] = b"bundle=";
    // Read one byte beyond the declaration so SPI truncation cannot make an
    // overlong macro look exact. The SPI limit excludes its NUL terminator.
    let required = PREFIX.len() + length + 2;
    crate::metadata_memory::admit(
        required as u128 + args.len() as u128 * std::mem::size_of::<String>() as u128
            + args.iter().map(|value| value.capacity() as u128).sum::<u128>(),
        fereg_core::memory::usable_memory_bytes(imposed_memory_limit_bytes()?),
    )?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(required)
        .map_err(|_| allocation("unable to reserve local estimation bundle metadata"))?;
    if bytes.capacity() < required {
        return Err(allocation(
            "local estimation bundle reservation was undersized",
        ));
    }
    bytes.resize(required, u8::MAX);
    bytes[..PREFIX.len()].copy_from_slice(PREFIX);
    spi.read_estimate_bundle(&mut bytes[PREFIX.len()..])?;
    if bytes[PREFIX.len()..].iter().position(|&byte| byte == 0) != Some(length) {
        return Err(syntax(
            "local estimation bundle byte count or terminator is inconsistent",
        ));
    }
    bytes.truncate(PREFIX.len() + length);
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| syntax("local estimation bundle is not valid UTF-8"))
}

/// Stata 18 rejects a plugin call with the complete guarded estimate protocol
/// as "too many options" before native dispatch.  Keep the ordinary argv form
/// for the C and Fake SPI boundaries, and additionally accept one explicitly
/// tagged, byte-length-framed bundle for the hidden licensed-Stata caller.
/// Decoding is complete and canonical before the handshake and before any SPI
/// metadata or observation read.
pub(crate) fn parse_estimate_key_values(args: &[String]) -> Result<BTreeMap<String, String>> {
    if args.len() > 85 {
        return Err(syntax("guarded OLS protocol has more than 85 fields"));
    }
    let Some(bundle) = (args.len() == 1)
        .then(|| args[0].strip_prefix("bundle="))
        .flatten()
    else {
        if args.iter().any(|argument| argument.starts_with("bundle=")) {
            return Err(syntax(
                "guarded OLS bundle must be the only action argument",
            ));
        }
        return parse_key_values(args);
    };
    let fields = decode_estimate_bundle(bundle)?;
    parse_key_values(&fields)
}

pub(crate) fn decode_estimate_bundle(bundle: &str) -> Result<Vec<String>> {
    const MAX_FIELDS: usize = 85;
    let bytes = bundle.as_bytes();
    let (count, mut cursor) = parse_bundle_decimal(bytes, 0, "field count")?;
    if count == 0 || count > MAX_FIELDS {
        return Err(syntax(
            "guarded OLS bundle field count must be between one and 85",
        ));
    }
    let mut fields = reserved_empty(count, "guarded OLS bundle fields")?;
    for _ in 0..count {
        let (length, payload_start) = parse_bundle_decimal(bytes, cursor, "field length")?;
        if length == 0 {
            return Err(syntax("guarded OLS bundle field is empty"));
        }
        let payload_end = payload_start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| syntax("guarded OLS bundle field is truncated"))?;
        let payload = bundle
            .get(payload_start..payload_end)
            .ok_or_else(|| syntax("guarded OLS bundle length splits UTF-8 data"))?;
        let mut field = String::new();
        field
            .try_reserve_exact(payload.len())
            .map_err(|_| allocation("unable to reserve guarded OLS bundle field"))?;
        if field.capacity() < payload.len() {
            return Err(allocation(
                "guarded OLS bundle field reservation was undersized",
            ));
        }
        field.push_str(payload);
        push_reserved(
            &mut fields,
            field,
            "guarded OLS bundle field capacity exhausted",
        )?;
        cursor = payload_end;
    }
    if cursor != bytes.len() {
        return Err(syntax("guarded OLS bundle has trailing data"));
    }
    Ok(fields)
}

fn parse_bundle_decimal(bytes: &[u8], start: usize, label: &str) -> Result<(usize, usize)> {
    let mut cursor = start;
    while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
        cursor += 1;
    }
    if cursor == start || cursor >= bytes.len() || bytes[cursor] != b':' {
        return Err(syntax(format!(
            "guarded OLS bundle {label} lacks a decimal length delimiter"
        )));
    }
    if cursor - start > 1 && bytes[start] == b'0' {
        return Err(syntax(format!(
            "guarded OLS bundle {label} is not canonically encoded"
        )));
    }
    let value = std::str::from_utf8(&bytes[start..cursor])
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| syntax(format!("guarded OLS bundle {label} overflows")))?;
    Ok((value, cursor + 1))
}

fn validate_cluster_request_shape(request: &OlsClusteredRequestV1) -> Result<()> {
    if !(1..=10).contains(&request.clusters.len()) {
        return Err(invalid(
            "clustered prepared OLS requires between one and ten dimensions",
        ));
    }
    for cluster in &request.clusters {
        if cluster.label.is_empty() || cluster.sources.is_empty() {
            return Err(invalid("clustered prepared OLS term is incomplete"));
        }
        let mut seen = BTreeSet::new();
        for source in &cluster.sources {
            if !seen.insert(source.get())
                || !request
                    .ols
                    .model
                    .sources
                    .iter()
                    .any(|entry| entry.id == *source)
            {
                return Err(invalid("clustered prepared OLS source is invalid"));
            }
        }
    }
    Ok(())
}

fn validate_vce(
    vce: OlsVceKind,
    clusters: &[OlsClusterTermV1],
    account_cluster_nesting: bool,
) -> Result<()> {
    match vce {
        OlsVceKind::Clustered if !(1..=10).contains(&clusters.len()) => Err(invalid(
            "clustered OLS requires between one and ten declared dimensions",
        )),
        OlsVceKind::Clustered if !account_cluster_nesting => Err(unsupported(
            "the guarded clustered OLS action requires cluster-aware FE nesting",
        )),
        OlsVceKind::Iid | OlsVceKind::Robust if !clusters.is_empty() => {
            Err(invalid("nonclustered OLS cannot receive cluster metadata"))
        }
        OlsVceKind::Iid | OlsVceKind::Robust if account_cluster_nesting => Err(invalid(
            "cluster nesting cannot be enabled without clustered inference",
        )),
        _ => Ok(()),
    }
}

fn read_clusters(spec: &EstimateActionSpec, spi: &mut dyn SpiApi) -> Result<Vec<OlsClusterTermV1>> {
    let terms = read_table(
        spi,
        &spec.cluster_term_matrix,
        spec.cluster_term_count,
        CLUSTER_TERM_ROW_WIDTH,
        "cluster term",
    )?;
    let components = read_table(
        spi,
        &spec.cluster_component_matrix,
        spec.cluster_component_count,
        CLUSTER_COMPONENT_ROW_WIDTH,
        "cluster component",
    )?;
    if spec.cluster_term_count == 0 {
        if spec.cluster_component_count != 0 {
            return Err(invalid("empty cluster terms have nonempty components"));
        }
        if terms.rows[0].iter().any(|value| *value != 0.0)
            || components.rows[0].iter().any(|value| *value != 0.0)
        {
            return Err(invalid(
                "empty cluster metadata requires exact zero sentinel rows",
            ));
        }
        return Ok(Vec::new());
    }
    let mut output = reserved_empty(spec.cluster_term_count, "cluster term table")?;
    let mut component_cursor = 0_usize;
    for term_index in 0..spec.cluster_term_count {
        let row = &terms.rows[term_index];
        let term = exact_usize(row[0], "cluster term id")?;
        let count = exact_usize(row[1], "cluster source count")?;
        if term != term_index + 1 || count == 0 {
            return Err(invalid("cluster term order or source count is invalid"));
        }
        if count > spec.cluster_component_count.saturating_sub(component_cursor) {
            return Err(invalid("cluster component table ended before declared source counts"));
        }
        let mut sources = reserved_empty(count, "cluster source recipe")?;
        for order in 1..=count {
            let component = components.rows.get(component_cursor).ok_or_else(|| {
                invalid("cluster component table ended before declared source counts")
            })?;
            if exact_usize(component[0], "cluster component term")? != term
                || exact_usize(component[1], "cluster component order")? != order
            {
                return Err(invalid("cluster component ordering is invalid"));
            }
            let source = exact_positive_u32(component[2], "cluster source id")?;
            push_reserved(
                &mut sources,
                SourceId::new(source)?,
                "cluster source recipe capacity exhausted",
            )?;
            component_cursor += 1;
        }
        push_reserved(
            &mut output,
            OlsClusterTermV1 {
                label: spec.cluster_labels[term_index].clone(),
                sources,
            },
            "cluster term table capacity exhausted",
        )?;
    }
    if component_cursor != spec.cluster_component_count {
        return Err(invalid("cluster component table has trailing rows"));
    }
    Ok(output)
}

#[cfg(test)]
pub(crate) fn load_ols_input(
    request: &OlsRequestV1,
    candidate_all: bool,
    input_complete: bool,
    spi: &mut dyn SpiApi,
) -> Result<LoadedOlsInput> {
    let mut memory = loading_memory(request, spi.nobs()?, 0, 0, None)?;
    load_ols_input_with_memory(request, candidate_all, input_complete, &mut memory, spi)
}

pub(crate) fn loading_memory(
    request: &OlsRequestV1,
    rows: usize,
    extra_columns: usize,
    parent_bytes: u128,
    usable_bytes: Option<u128>,
) -> Result<InputMemory> {
    use crate::input_memory::{add, mul};
    use std::mem::size_of;
    let sources = request.model.sources.len();
    let weights = request.bindings.iter().filter(|binding| matches!(binding.role,
        OlsInputRole::AnalyticWeight | OlsInputRole::FrequencyWeight | OlsInputRole::ProbabilityWeight)).count();
    let columns = sources.checked_add(weights).and_then(|v| v.checked_add(extra_columns))
        .and_then(|v| v.checked_add(1)).ok_or_else(|| allocation("input column count overflows"))?;
    let row_bytes = add(mul(columns, 9)?, 1)?;
    let arrays = (rows as u128).checked_mul(row_bytes).ok_or_else(|| allocation("input array bytes overflow"))?;
    // The loader and final source descriptors can coexist. Position lists,
    // whole-column and chunk slices, plus the SPI's positions/pointers are
    // column-sized, with at most one bulk call live on the Stata thread.
    let sources_bytes = mul(sources, size_of::<(SourceId, OlsSourceLoader)>()
        + size_of::<(SourceId, LoadedNumericColumn)>() + size_of::<u32>())?;
    let transport_bytes = mul(columns, 2 * size_of::<u32>() + size_of::<*mut f64>()
        + 2 * size_of::<&mut [f64]>() + 2 * size_of::<&mut [bool]>())?;
    InputMemory::new(usable_bytes, add(parent_bytes, add(arrays, add(sources_bytes, transport_bytes)?)?)?)
}

pub(crate) fn load_ols_input_with_memory(
    request: &OlsRequestV1,
    candidate_all: bool,
    input_complete: bool,
    memory: &mut InputMemory,
    spi: &mut dyn SpiApi,
) -> Result<LoadedOlsInput> {
    load_ols_input_with_team(request, candidate_all, input_complete, memory, spi, None)
}

fn load_ols_input_with_team(
    request: &OlsRequestV1,
    candidate_all: bool,
    input_complete: bool,
    memory: &mut InputMemory,
    spi: &mut dyn SpiApi,
    team: Option<ThreadTeam>,
) -> Result<LoadedOlsInput> {
    let n = spi.nobs()?;
    let mut candidate = reserved_filled(n, candidate_all, "OLS candidate marker")?;
    let candidate_position = if candidate_all {
        None
    } else {
        Some(role_position(request, InputRoleTag::Candidate)?)
    };
    let mut outcome = empty_loaded_column(n, "OLS outcome")?;
    let mut source_loaders = reserved_empty(request.model.sources.len(), "OLS source table")?;
    for source in &request.model.sources {
        let position = source_position(request, source.id)?;
        let loader = if spi.variable_is_string(position)? {
            if source_requires_numeric(request, source.id) {
                return Err(unsupported(format!(
                    "model source {} must be numeric",
                    source.id.get()
                )));
            }
            if spi.variable_is_strl(position)? {
                return Err(unsupported(format!(
                    "model source {} uses unsupported strL storage",
                    source.id.get()
                )));
            }
            OlsSourceLoader::FixedWidthString {
                column: empty_loaded_column(n, "OLS string source codes")?,
                provisional_codes: StringDictionary::new(),
            }
        } else {
            OlsSourceLoader::Numeric(empty_loaded_column(n, "OLS numeric source")?)
        };
        push_reserved(
            &mut source_loaders,
            (source.id, loader),
            "OLS source table capacity exhausted",
        )?;
    }
    let outcome_position = role_position(request, InputRoleTag::Outcome)?;
    let analytic_position = weight_position(request, OlsInputRole::AnalyticWeight);
    let frequency_position = weight_position(request, OlsInputRole::FrequencyWeight);
    let probability_position = weight_position(request, OlsInputRole::ProbabilityWeight);
    let mut analytic_weight = analytic_position
        .map(|_| empty_loaded_column(n, "analytic weight"))
        .transpose()?;
    let mut frequency_weight = frequency_position
        .map(|_| empty_loaded_column(n, "frequency weight"))
        .transpose()?;
    let mut probability_weight = probability_position
        .map(|_| empty_loaded_column(n, "probability weight"))
        .transpose()?;
    let mut source_positions = reserved_empty(request.model.sources.len(), "OLS source positions")?;
    for source in &request.model.sources {
        push_reserved(
            &mut source_positions,
            source_position(request, source.id)?,
            "OLS source-position capacity exhausted",
        )?;
    }
    let all_sources_numeric = source_loaders
        .iter()
        .all(|(_, loader)| matches!(loader, OlsSourceLoader::Numeric(_)));
    if candidate_all && all_sources_numeric || n >= INPUT_CANDIDATE_BATCH_MIN_ROWS {
        if let Some(position) = candidate_position {
            for (row, retained) in candidate.iter_mut().enumerate() {
                if row % INPUT_INTERRUPT_POLL_STRIDE == 0 {
                    spi.check_interrupted()?;
                }
                *retained = read_candidate_marker(spi, position, row + 1)?;
            }
        }
        let mut positions = reserved_empty(
            1 + source_positions.len()
                + usize::from(analytic_position.is_some())
                + usize::from(frequency_position.is_some())
                + usize::from(probability_position.is_some()),
            "numeric SPI positions",
        )?;
        let mut columns: Vec<&mut [f64]> = Vec::new();
        columns
            .try_reserve_exact(positions.capacity())
            .map_err(|_| allocation("unable to reserve numeric SPI column batch"))?;
        let mut missing: Vec<&mut [bool]> =
            reserved_empty(positions.capacity(), "numeric SPI missing columns")?;
        positions.push(outcome_position);
        missing.push(outcome.missing.as_mut_slice());
        columns.push(outcome.values.as_mut_slice());
        for ((_, destination), position) in source_loaders.iter_mut().zip(&source_positions) {
            let OlsSourceLoader::Numeric(destination) = destination else {
                continue;
            };
            positions.push(*position);
            columns.push(destination.values.as_mut_slice());
            missing.push(destination.missing.as_mut_slice());
        }
        for (position, destination) in [
            (analytic_position, analytic_weight.as_mut()),
            (frequency_position, frequency_weight.as_mut()),
            (probability_position, probability_weight.as_mut()),
        ] {
            if let (Some(position), Some(destination)) = (position, destination) {
                positions.push(position);
                columns.push(destination.values.as_mut_slice());
                missing.push(destination.missing.as_mut_slice());
            }
        }
        let pipelined = if candidate_all && all_sources_numeric {
            if let Some(team) = team {
                crate::numeric_pipeline::try_read(spi, &positions, &mut columns, &mut missing, team, memory)?
            } else { false }
        } else { false };
        if !pipelined {
            read_numeric_columns_in_chunks(
                spi,
                &positions,
                &mut columns,
                &mut missing,
                (!candidate_all).then_some(candidate.as_slice()),
            )?;
        }
        if candidate_all
            && all_sources_numeric
            && input_complete
            && missing.iter().any(|flags| flags.iter().any(|flag| *flag))
        {
            return Err(invalid_data(
                "Stata completeness certificate disagrees with numeric input",
            ));
        }
        drop(columns);
        drop(missing);
        // String dictionaries keep the original encounter order within each
        // source. Numeric sources need not lose batching because a peer is a
        // string; excluded rows never reach either kind of SPI read.
        if !all_sources_numeric {
            for ((_, loader), position) in source_loaders.iter_mut().zip(&source_positions) {
                if !matches!(loader, OlsSourceLoader::FixedWidthString { .. }) {
                    continue;
                }
                for (row, retained) in candidate.iter().copied().enumerate() {
                    if row % INPUT_INTERRUPT_POLL_STRIDE == 0 {
                        spi.check_interrupted()?;
                    }
                    if retained {
                        loader.read_into(spi, *position, row + 1, row, memory)?;
                    }
                }
            }
        }
    } else {
        for (zero_based, retained_slot) in candidate.iter_mut().enumerate() {
            if zero_based % INPUT_INTERRUPT_POLL_STRIDE == 0 {
                spi.check_interrupted()?;
            }
            let observation = zero_based + 1;
            let retained_candidate = if let Some(candidate_position) = candidate_position {
                let retained = read_candidate_marker(spi, candidate_position, observation)?;
                *retained_slot = retained;
                retained
            } else {
                true
            };
            if !retained_candidate {
                continue;
            }
            read_into(spi, outcome_position, observation, &mut outcome, zero_based)?;
            for ((_, destination), position) in source_loaders.iter_mut().zip(&source_positions) {
                destination.read_into(spi, *position, observation, zero_based, memory)?;
            }
            if let (Some(position), Some(destination)) =
                (analytic_position, analytic_weight.as_mut())
            {
                read_into(spi, position, observation, destination, zero_based)?;
            }
            if let (Some(position), Some(destination)) =
                (frequency_position, frequency_weight.as_mut())
            {
                read_into(spi, position, observation, destination, zero_based)?;
            }
            if let (Some(position), Some(destination)) =
                (probability_position, probability_weight.as_mut())
            {
                read_into(spi, position, observation, destination, zero_based)?;
            }
        }
    }
    spi.check_interrupted()?;
    let mut numeric_sources = reserved_empty(source_loaders.len(), "final OLS source table")?;
    for (source, loader) in source_loaders {
        push_reserved(
            &mut numeric_sources,
            (source, loader.finalize(memory)?),
            "final OLS source table capacity exhausted",
        )?;
    }
    Ok(LoadedOlsInput {
        candidate,
        outcome,
        numeric_sources,
        analytic_weight,
        frequency_weight,
        probability_weight,
    })
}

fn read_candidate_marker(spi: &mut dyn SpiApi, position: u32, observation: usize) -> Result<bool> {
    let (value, missing) = spi.read_numeric(position, observation)?;
    if missing || !value.is_finite() || !matches!(value, 0.0 | 1.0) {
        return Err(invalid_data(format!(
            "candidate marker at observation {observation} must be nonmissing zero or one",
        )));
    }
    Ok(value == 1.0)
}

pub(crate) fn read_numeric_columns_in_chunks(
    spi: &mut dyn SpiApi,
    positions: &[u32],
    columns: &mut [&mut [f64]],
    missing: &mut [&mut [bool]],
    candidate: Option<&[bool]>,
) -> Result<()> {
    if positions.len() != columns.len()
        || missing.len() != columns.len()
        || columns.is_empty()
        || columns
            .iter()
            .zip(missing.iter())
            .any(|(column, flags)| column.len() != columns[0].len() || flags.len() != column.len())
        || candidate.is_some_and(|rows| rows.len() != columns[0].len())
    {
        return Err(invalid("numeric SPI column batch has an invalid shape"));
    }
    let rows = columns[0].len();
    let mut start = 0;
    let mut next_poll = 0;
    while start < rows {
        if start >= next_poll {
            spi.check_interrupted()?;
            next_poll = start.saturating_add(INPUT_INTERRUPT_POLL_STRIDE);
        }
        if let Some(candidate) = candidate {
            if !candidate[start] {
                start += 1;
                while start < rows && !candidate[start] {
                    if start >= next_poll {
                        spi.check_interrupted()?;
                        next_poll = start.saturating_add(INPUT_INTERRUPT_POLL_STRIDE);
                    }
                    start += 1;
                }
                continue;
            }
        }
        let mut end = start.saturating_add(INPUT_BULK_READ_CHUNK_ROWS).min(rows);
        if let Some(candidate) = candidate {
            if let Some(excluded) = candidate[start..end].iter().position(|retained| !*retained) {
                end = start + excluded;
            }
        }
        // Fragmented samples must not pay a set of FFI/slice allocations for
        // every isolated row. Long candidate runs use the same bounded raw
        // double reader as unrestricted input, with no packing or excluded reads.
        if candidate.is_some() && end - start < INPUT_BULK_MIN_RUN_ROWS {
            for row in start..end {
                for ((position, column), flags) in positions
                    .iter()
                    .zip(columns.iter_mut())
                    .zip(missing.iter_mut())
                {
                    let (value, is_missing) = spi.read_numeric(*position, row + 1)?;
                    column[row] = value;
                    flags[row] = is_missing;
                }
            }
        } else {
            let mut chunk = reserved_empty(columns.len(), "numeric SPI chunk")?;
            chunk.extend(columns.iter_mut().map(|column| &mut column[start..end]));
            let mut flags = reserved_empty(missing.len(), "numeric SPI missing chunk")?;
            flags.extend(missing.iter_mut().map(|column| &mut column[start..end]));
            spi.read_numeric_columns(positions, start + 1, &mut chunk, &mut flags)?;
        }
        start = end;
    }
    Ok(())
}

enum OlsSourceLoader {
    Numeric(LoadedNumericColumn),
    FixedWidthString {
        column: LoadedNumericColumn,
        provisional_codes: StringDictionary,
    },
}

impl OlsSourceLoader {
    fn read_into(
        &mut self,
        spi: &mut dyn SpiApi,
        position: u32,
        observation: usize,
        zero_based: usize,
        memory: &mut InputMemory,
    ) -> Result<()> {
        match self {
            Self::Numeric(column) => read_into(spi, position, observation, column, zero_based),
            Self::FixedWidthString { column, provisional_codes } => {
                let value = spi.read_string_with_limit(position, observation, memory.remaining())?;
                memory.admit(value.capacity() as u128)?;
                if value.is_empty() {
                    column.missing[zero_based] = true;
                    return Ok(());
                }
                column.values[zero_based] = f64::from(provisional_codes.encode(value, memory)?);
                column.missing[zero_based] = false;
                Ok(())
            }
        }
    }

    fn finalize(self, memory: &mut InputMemory) -> Result<LoadedNumericColumn> {
        match self {
            Self::Numeric(column) => Ok(column),
            Self::FixedWidthString { mut column, provisional_codes } => {
                provisional_codes.recode(&mut column.values, &column.missing, memory)?;
                Ok(column)
            }
        }
    }
}

fn read_into(
    spi: &mut dyn SpiApi,
    position: u32,
    observation: usize,
    destination: &mut LoadedNumericColumn,
    zero_based: usize,
) -> Result<()> {
    let (value, missing) = spi.read_numeric(position, observation)?;
    destination.values[zero_based] = value;
    destination.missing[zero_based] = missing;
    Ok(())
}

fn build_derived_observation_outputs(
    prepared: PreparedOlsResultData<'_>,
    categorical: Option<&PreparedCategoricalOlsData>,
    fit: &CertifiedPreparedOlsFit,
    requested: &ObservationOutputs,
    known_connected: bool,
    fit_config: PreparedOlsFitConfig,
) -> Result<DerivedObservationOutputs> {
    ThreadTeam::new(fit_config.execution.threads).install(|| {
        build_derived_observation_outputs_in_team(
            prepared,
            categorical,
            fit,
            requested,
            known_connected,
            fit_config,
        )
    })?
}

fn build_derived_observation_outputs_in_team(
    prepared: PreparedOlsResultData<'_>,
    categorical: Option<&PreparedCategoricalOlsData>,
    fit: &CertifiedPreparedOlsFit,
    requested: &ObservationOutputs,
    known_connected: bool,
    fit_config: PreparedOlsFitConfig,
) -> Result<DerivedObservationOutputs> {
    let any_derived = requested.xb.is_some()
        || requested.xbd.is_some()
        || requested.absorbed.is_some()
        || requested.saved_channels.iter().any(Option::is_some);
    if !any_derived {
        return Ok(DerivedObservationOutputs {
            xb: None,
            xbd: None,
            absorbed: None,
            saved_channels: reserved_filled(
                requested.saved_channels.len(),
                None,
                "derived saved-output registry",
            )?,
        });
    }
    let n = prepared.sample().n_active();
    if fit.residuals().len() != n || fit.coefficients().len() != prepared.model().coefficients.len()
    {
        return Err(invalid(
            "prepared fit and derived-output model dimensions differ",
        ));
    }
    let needs_absorbed =
        requested.absorbed.is_some() || requested.saved_channels.iter().any(Option::is_some);
    let materialize_xb = requested.xb.is_some() || requested.absorbed.is_some();
    let mut fitted_terms = reserved_empty(fit.coefficients().len(), "derived OLS fitted terms")?;
    for fitted in fit.coefficients() {
        match (fitted.generated_column_index, fitted.value) {
            (Some(column), Some(coefficient)) => push_reserved(
                &mut fitted_terms,
                (column, coefficient),
                "derived OLS fitted-term capacity exhausted",
            )?,
            (None, None) => {}
            (Some(_), None)
                if matches!(
                    fitted.status,
                    PreparedOlsFitStatus::CollinearWithFixedEffects
                        | PreparedOlsFitStatus::NumericallyRankDeficient
                ) => {}
            _ => {
                return Err(invalid(
                    "derived OLS output coefficient identity is incomplete",
                ))
            }
        }
    }
    for fitted in fit.coefficients() {
        if fitted.value.is_some()
            && !matches!(
                fitted.status,
                PreparedOlsFitStatus::Kept | PreparedOlsFitStatus::NormalizedAbsorbedIntercept
            )
        {
            return Err(invalid(
                "derived OLS output received a value for a nonfitted coefficient",
            ));
        }
    }
    let mut xb_work = if materialize_xb {
        Some(reserved_filled(n, 0.0, "derived OLS xb output")?)
    } else {
        None
    };
    if let Some(xb) = xb_work.as_mut() {
        xb.par_iter_mut()
            .enumerate()
            .try_for_each(|(row, value)| -> Result<()> {
                for &(column, coefficient) in &fitted_terms {
                    *value = coefficient.mul_add(prepared.column_value(column, row)?, *value);
                }
                Ok(())
            })?;
    }
    if xb_work
        .as_ref()
        .is_some_and(|values| values.iter().any(|value| !value.is_finite()))
    {
        return Err(invalid_data("derived OLS xb output is not finite"));
    }
    let mut absorbed_target = if needs_absorbed {
        Some(reserved_filled(n, 0.0, "derived OLS absorbed target")?)
    } else {
        None
    };
    if let Some(target) = absorbed_target.as_mut() {
        if let Some(xb) = xb_work.as_ref() {
            target.par_iter_mut().enumerate().for_each(|(row, value)| {
                *value = prepared.sample().outcome[row] - xb[row] - fit.residuals()[row];
            });
        } else {
            // Saved-only publication does not need an xb vector after target
            // construction. Accumulate xb in the target buffer, then convert
            // it in place; this preserves the ordinary xb rounding exactly.
            target
                .par_iter_mut()
                .enumerate()
                .try_for_each(|(row, value)| -> Result<()> {
                    let mut xb = 0.0_f64;
                    for &(column, coefficient) in &fitted_terms {
                        xb = coefficient.mul_add(prepared.column_value(column, row)?, xb);
                    }
                    *value = prepared.sample().outcome[row] - xb - fit.residuals()[row];
                    Ok(())
                })?;
        }
    }
    if absorbed_target
        .as_ref()
        .is_some_and(|values| values.iter().any(|value| !value.is_finite()))
    {
        return Err(invalid_data("derived OLS absorbed target is not finite"));
    }

    let mut saved_channels = reserved_empty(
        requested.saved_channels.len(),
        "derived saved-output registry",
    )?;
    let mut recovered_absorbed = None;
    if requested.saved_channels.iter().any(Option::is_some) {
        let categorical = categorical.ok_or_else(|| {
            invalid("saved fixed-effect recovery requires categorical prepared topology")
        })?;
        let mut mask = reserved_empty(
            requested.saved_channels.len(),
            "derived saved-output request mask",
        )?;
        for position in &requested.saved_channels {
            push_reserved(
                &mut mask,
                position.is_some(),
                "derived saved-output request mask capacity exhausted",
            )?;
        }
        // The recovery model already charges returned channels and its own
        // scratch. Only these caller-owned buffers coexist with that attempt;
        // xbd and the publication matrices are constructed after it returns.
        let parent_bytes = (xb_work.as_ref().map_or(0, Vec::capacity) as u128
            + absorbed_target.as_ref().map_or(0, Vec::capacity) as u128)
            * std::mem::size_of::<f64>() as u128
            + fitted_terms.capacity() as u128 * std::mem::size_of::<(usize, f64)>() as u128
            + saved_channels.capacity() as u128 * std::mem::size_of::<Option<Vec<f64>>>() as u128
            + mask.capacity() as u128 * std::mem::size_of::<bool>() as u128;
        let recovery_budget = fit_config
            .absorb
            .solver_memory_budget
            .map(|budget| {
                budget
                    .execution_bytes
                    .checked_add(parent_bytes)
                    .map(|execution_bytes| FeSolverMemoryBudget {
                        execution_bytes,
                        ..budget
                    })
                    .ok_or_else(|| allocation("OLS recovery parent byte count overflows"))
            })
            .transpose()?;
        let recovered = categorical.recover_absorbed_effects_fast(
            absorbed_target
                .as_deref()
                .ok_or_else(|| invalid("saved fixed-effect recovery target is absent"))?,
            &mask,
            fit.recovered_fe_coefficients(),
            known_connected,
            requested.absorbed.is_some(),
            LsmrConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                ..fit_config.absorb.lsmr
            },
            MapConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                check_every: 1,
                symmetric: true,
            },
            recovery_budget,
        )?;
        let (absorbed_sum, channels) = recovered.into_optional_parts();
        recovered_absorbed = absorbed_sum;
        saved_channels = channels;
    } else {
        saved_channels.resize_with(requested.saved_channels.len(), || None);
    }

    let xbd = if requested.xbd.is_some() {
        let mut values = reserved_filled(n, 0.0, "derived OLS xbd output")?;
        values.par_iter_mut().enumerate().for_each(|(row, value)| {
            *value = prepared.sample().outcome[row] - fit.residuals()[row];
        });
        Some(values)
    } else {
        None
    };
    let absorbed = if requested.absorbed.is_some() {
        Some(
            recovered_absorbed
                .or(absorbed_target)
                .ok_or_else(|| invalid("requested absorbed output is absent"))?,
        )
    } else {
        None
    };
    let xb = if requested.xb.is_some() {
        xb_work
    } else {
        None
    };
    Ok(DerivedObservationOutputs {
        xb,
        xbd,
        absorbed,
        saved_channels,
    })
}

fn normalization_known_connected(report: &CategoricalTopologyDofReport) -> bool {
    // Under first-pair/pairwise DoF, the second eligible intercept's exact
    // redundancy is the number of components in the first two intercepts'
    // bipartite graph. One component there also connects every later effect,
    // because each retained observation supplies a level in every effect.
    let mut intercepts = report
        .effects
        .iter()
        .filter(|effect| effect.slope_source.is_none());
    let Some(first) = intercepts.next() else {
        return false;
    };
    let Some(second) = intercepts.next() else {
        return false;
    };
    !first.nested && !second.nested && second.redundancy_exact && second.redundant == 1
}

fn precommit_and_store(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
    derived: &DerivedObservationOutputs,
    observation: &ActionObservation,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    let expected_mode = match action.memory_plan.selected {
        MemoryMode::Fast => PreparedOlsExecutionMode::Fast,
        MemoryMode::Low => PreparedOlsExecutionMode::Low,
        MemoryMode::Auto => {
            return Err(invalid(
                "memory preflight retained an unresolved automatic mode",
            ));
        }
    };
    if shadow.execution.mode != expected_mode
        || shadow.execution.poolsize != action.fit.execution.poolsize
        || shadow.execution.requested_threads
            != action
                .fit
                .execution
                .threads
                .requested
                .map_or(0, |value| value.get())
        || shadow.execution.effective_threads != action.expected_effective_threads
        || shadow.execution.deterministic != action.fit.execution.threads.deterministic
        || (shadow.execution.scratch_bytes > 0)
            != (expected_mode == PreparedOlsExecutionMode::Low
                && action.memory_plan.estimate.recommended_poolsize > 0)
    {
        return Err(invalid(
            "native OLS execution report disagrees with memory preflight",
        ));
    }
    let runtime = build_runtime_shadow(action, shadow, observation)?;
    let publication_bytes = shadow.capacity_bytes()
        + derived.capacity_bytes()
        + runtime.capacity() as u128 * std::mem::size_of::<f64>() as u128;
    if shadow.capacity_bytes() > action.publication_memory.shadow_bytes
        || publication_bytes > action.publication_memory.publication_bytes
    {
        return Err(allocation(
            "OLS publication capacities exceed preflight admission",
        ));
    }
    if std::env::var_os("FEREG_ACTION_MEMORY_AUDIT").is_some() {
        let resources = instrumentation::snapshot();
        crate::action_memory::emit(&action.memory_ledger(shadow.solver_workspace_peak)?,
            resources.allocation_peak_bytes.saturating_sub(observation.resources_start.allocation_current_bytes))?;
    }
    let forecast = action.memory_forecast(shadow)?;
    if let Some(forecast) = forecast {
        forecast.trace(action.memory_baseline, instrumentation::native_live_before())?;
    }
    let memory_report = MemoryReport::estimated(action.memory_baseline,
        instrumentation::native_live_before(),
        action.memory_ledger(shadow.solver_workspace_peak)?.summary(),
        action.memory_plan.safety_headroom_bytes, forecast)?;
    validate_shadow_against_outputs(action, shadow)?;
    validate_derived_against_outputs(action, shadow, derived)?;
    validate_defined_value_table(&runtime, "OLS runtime shadow")?;
    spi.check_interrupted()?;
    store_shadow(action, shadow, derived, &runtime, &memory_report, spi)
}

fn build_runtime_shadow(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
    observation: &ActionObservation,
) -> Result<Vec<f64>> {
    let resources = instrumentation::snapshot();
    let total_seconds = observation.started.elapsed().as_secs_f64();
    let native_seconds =
        (total_seconds - observation.preflight_seconds - observation.input_load_seconds).max(0.0);
    let allocation_peak_delta = resources
        .allocation_peak_bytes
        .saturating_sub(observation.resources_start.allocation_current_bytes);
    let selected_bytes = action.memory_plan.selected_bytes();
    let modeled_allocation_ratio =
        (selected_bytes > 0).then_some(allocation_peak_delta as f64 / selected_bytes as f64);
    let rss_delta = match (
        observation.resources_start.rss_current_bytes,
        resources.rss_current_bytes,
    ) {
        (Some(start), Some(end)) => Some(end as f64 - start as f64),
        _ => None,
    };
    let effective_tolerance = action.fit.absorb.map.tolerance;
    let rows = [
        Some(memory_mode_code(action.memory_plan.requested)),
        Some(memory_mode_code(action.memory_plan.selected)),
        Some(action.fit.execution.poolsize as f64),
        Some(action.memory_plan.estimate.fast_bytes as f64),
        Some(action.memory_plan.estimate.low_bytes as f64),
        action.memory_plan.available_bytes.map(|value| value as f64),
        Some(action.memory_plan.safety_headroom_bytes as f64),
        action.memory_plan.usable_bytes.map(|value| value as f64),
        Some(shadow.execution.scratch_bytes as f64),
        Some(
            observation
                .resources_start
                .allocation_previous_baseline_bytes as f64,
        ),
        Some(observation.resources_start.allocation_current_bytes as f64),
        Some(resources.allocation_peak_bytes as f64),
        Some(allocation_peak_delta as f64),
        observation
            .resources_start
            .rss_current_bytes
            .map(|value| value as f64),
        resources.rss_current_bytes.map(|value| value as f64),
        observation
            .resources_start
            .rss_peak_bytes
            .map(|value| value as f64),
        resources.rss_peak_bytes.map(|value| value as f64),
        Some(observation.preflight_seconds),
        Some(observation.input_load_seconds),
        Some(native_seconds),
        Some(total_seconds),
        Some(action.requested_tolerance),
        Some(effective_tolerance),
        Some(action.fit.absorb.map.max_iterations as f64),
        Some(selected_bytes as f64),
        modeled_allocation_ratio,
        rss_delta,
        Some(shadow.execution.requested_threads as f64),
        Some(shadow.execution.effective_threads as f64),
        Some(if shadow.execution.deterministic {
            1.0
        } else {
            0.0
        }),
    ];
    let mut runtime = reserved_filled(RUNTIME_ROWS * 2, 0.0, "OLS runtime shadow")?;
    for (row, value) in rows.into_iter().enumerate() {
        if let Some(value) = value {
            if !value.is_finite() {
                return Err(invalid("OLS runtime observation is nonfinite"));
            }
            runtime[row * 2] = 1.0;
            runtime[row * 2 + 1] = value;
        }
    }
    Ok(runtime)
}

const fn memory_mode_code(mode: MemoryMode) -> f64 {
    match mode {
        MemoryMode::Fast => 1.0,
        MemoryMode::Low => 2.0,
        MemoryMode::Auto => 3.0,
    }
}

fn validate_derived_against_outputs(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
    derived: &DerivedObservationOutputs,
) -> Result<()> {
    let n = shadow.retained_rows.len();
    for (position, values) in [
        (action.outputs.xb, derived.xb.as_deref()),
        (action.outputs.xbd, derived.xbd.as_deref()),
        (action.outputs.absorbed, derived.absorbed.as_deref()),
    ] {
        if position.is_some() != values.is_some()
            || values.is_some_and(|values| {
                values.len() != n || values.iter().any(|value| !value.is_finite())
            })
        {
            return Err(invalid(
                "derived OLS observation output failed final preflight",
            ));
        }
    }
    if derived.saved_channels.len() != action.outputs.saved_channels.len() {
        return Err(invalid(
            "derived saved-output channel layout failed final preflight",
        ));
    }
    for (position, values) in action
        .outputs
        .saved_channels
        .iter()
        .zip(&derived.saved_channels)
    {
        if position.is_some() != values.is_some()
            || values.as_ref().is_some_and(|values| {
                values.len() != n || values.iter().any(|value| !value.is_finite())
            })
        {
            return Err(invalid(
                "derived saved fixed-effect output failed final preflight",
            ));
        }
    }
    if let (Some(xb), Some(absorbed), Some(xbd)) = (
        derived.xb.as_deref(),
        derived.absorbed.as_deref(),
        derived.xbd.as_deref(),
    ) {
        for row in 0..n {
            let error = (xb[row] + absorbed[row] - xbd[row]).abs();
            if error > 1e-9 * xbd[row].abs().max(1.0) {
                return Err(invalid_data(
                    "derived xb, absorbed, and xbd outputs disagree",
                ));
            }
        }
    }
    Ok(())
}

fn validate_shadow_against_outputs(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
) -> Result<()> {
    let matrix_dimension = shadow.coefficient_count.max(1);
    let matrix_capacity = shadow.coefficient_capacity.max(1);
    if shadow.b.len() != matrix_dimension
        || shadow.covariance.len() != matrix_dimension * matrix_dimension
        || shadow.coefficient_status.len() != matrix_dimension * COEFFICIENT_STATUS_COLUMNS
        || shadow.omission_b_marker.len() != matrix_dimension
        || shadow.omission_v_marker.len() != matrix_dimension * matrix_dimension
        || shadow.fit.len() != FIT_ROWS * 2
        || shadow.dof.len() != DOF_ROWS * 2
        || shadow.solver.len() != SOLVER_ROWS * SOLVER_COLUMNS
        || shadow.fixed_effects.len() != shadow.fixed_effect_rows * FE_COLUMNS
        || shadow.clusters.len() != shadow.cluster_rows * CLUSTER_COLUMNS
        || shadow.subsets.len() != shadow.subset_rows * SUBSET_COLUMNS
        || shadow.certificates.len() != CERTIFICATE_ROWS * 2
        || shadow.receipt.len() != RECEIPT_COLUMNS
        || shadow.drop_reasons.len() != shadow.receipt[6] as usize
        || shadow.retained_rows.len() != shadow.residuals.len()
        || shadow.covariance_dimension > shadow.coefficient_count
        || shadow.coefficient_count > shadow.coefficient_capacity
        || action.matrices.b.rows() != 1
        || action.matrices.b.columns() != matrix_capacity
        || action.matrices.covariance.rows() != matrix_capacity
        || action.matrices.covariance.columns() != matrix_capacity
        || action.matrices.coefficient_status.rows() != matrix_capacity
        || action.matrices.coefficient_status.columns() != COEFFICIENT_STATUS_COLUMNS
        || action.matrices.omission_b_marker.rows() != 1
        || action.matrices.omission_b_marker.columns() != matrix_capacity
        || action.matrices.omission_v_marker.rows() != matrix_capacity
        || action.matrices.omission_v_marker.columns() != matrix_capacity
    {
        return Err(invalid(
            "OLS shadow result failed its final dimensional preflight",
        ));
    }
    if shadow.residuals.iter().any(|value| !value.is_finite())
        || shadow.retained_rows.iter().enumerate().any(|(index, row)| {
            *row >= shadow.drop_reasons.len()
                || (index > 0 && shadow.retained_rows[index - 1] >= *row)
        })
        || !shadow
            .drop_reasons
            .iter()
            .enumerate()
            .filter_map(|(row, reason)| {
                (*reason == fereg_core::DropReason::Retained).then_some(row)
            })
            .eq(shadow.retained_rows.iter().copied())
    {
        return Err(invalid(
            "OLS shadow observation outputs failed final preflight",
        ));
    }
    if action.outputs.residual.is_some() != (shadow.receipt[13] == 1.0) || shadow.receipt[15] != 0.0
    {
        return Err(invalid(
            "OLS shadow receipt disagrees with requested outputs",
        ));
    }
    validate_shadow_receipt(action, shadow)?;
    validate_shadow_coefficients(shadow)?;
    validate_defined_value_table(&shadow.fit, "OLS fit-statistics shadow")?;
    validate_defined_value_table(&shadow.dof, "OLS DoF shadow")?;
    validate_defined_value_table(&shadow.certificates, "OLS certificate shadow")?;
    validate_shadow_diagnostics(action, shadow)?;
    Ok(())
}

fn validate_shadow_receipt(action: &EstimateAction, shadow: &OlsShadowResult<'_>) -> Result<()> {
    if shadow.receipt.iter().any(|value| !value.is_finite()) {
        return Err(invalid("OLS shadow receipt contains a nonfinite field"));
    }
    let logical_clusters = usize::from(action.vce == OlsVceKind::Clustered) * action.clusters.len();
    let logical_subsets = usize::from(action.vce == OlsVceKind::Clustered) * shadow.subset_rows;
    let expected = [
        RESULT_MAGIC,
        f64::from(OLS_RESULT_WIRE_VERSION.major),
        f64::from(OLS_RESULT_WIRE_VERSION.minor),
        action.vce.code(),
        shadow.coefficient_count as f64,
        shadow.covariance_dimension as f64,
        shadow.drop_reasons.len() as f64,
        shadow.dof[5],
        shadow.retained_rows.len() as f64,
        logical_clusters as f64,
        shadow.fixed_effect_rows as f64,
        logical_subsets as f64,
        1.0,
        f64::from(action.outputs.residual.is_some()),
        1.0,
        0.0,
    ];
    if shadow.receipt != expected {
        return Err(invalid(
            "OLS shadow receipt failed final native identity validation",
        ));
    }
    let candidate = exact_usize(shadow.receipt[7], "OLS candidate count")?;
    if candidate > shadow.drop_reasons.len() || shadow.retained_rows.len() > candidate {
        return Err(invalid("OLS shadow receipt sample counts are out of range"));
    }
    Ok(())
}

fn validate_shadow_coefficients(shadow: &OlsShadowResult<'_>) -> Result<()> {
    let dimension = shadow.coefficient_count.max(1);
    if shadow.coefficient_count == 0 {
        if shadow.covariance_dimension != 0
            || shadow.b != [0.0]
            || shadow.covariance != [0.0]
            || shadow.coefficient_status != [0.0; COEFFICIENT_STATUS_COLUMNS]
            || shadow.omission_b_marker != [0.0]
            || shadow.omission_v_marker != [0.0]
        {
            return Err(invalid("OLS zero-coordinate shadow sentinel is invalid"));
        }
        return Ok(());
    }

    let mut reported_coordinates = 0_usize;
    for row in 0..dimension {
        let offset = row * COEFFICIENT_STATUS_COLUMNS;
        let slot = exact_usize(shadow.coefficient_status[offset], "OLS coefficient slot")?;
        let code = exact_usize(
            shadow.coefficient_status[offset + 1],
            "OLS coefficient status",
        )?;
        let _generated = exact_usize(
            shadow.coefficient_status[offset + 2],
            "OLS generated-column status",
        )?;
        let coordinate = exact_usize(
            shadow.coefficient_status[offset + 3],
            "OLS covariance coordinate",
        )?;
        if slot != row + 1 || !(1..=5).contains(&code) || !shadow.b[row].is_finite() {
            return Err(invalid("OLS coefficient shadow identity is invalid"));
        }
        let reported = matches!(code, 1 | 5);
        if reported {
            reported_coordinates = reported_coordinates
                .checked_add(1)
                .ok_or_else(|| allocation("OLS reported-coordinate count overflows"))?;
            if coordinate == 0
                || coordinate > shadow.covariance_dimension
                || shadow.omission_b_marker[row] != 1.0
            {
                return Err(invalid("OLS reported covariance coordinate is invalid"));
            }
            let duplicates = (0..dimension)
                .filter(|other| {
                    exact_usize(
                        shadow.coefficient_status[other * COEFFICIENT_STATUS_COLUMNS + 3],
                        "OLS covariance coordinate",
                    )
                    .is_ok_and(|other_coordinate| other_coordinate == coordinate)
                })
                .count();
            if duplicates != 1 {
                return Err(invalid("OLS covariance coordinate is duplicated"));
            }
        } else if coordinate != 0 || shadow.b[row] != 0.0 || shadow.omission_b_marker[row] != 0.0 {
            return Err(invalid("OLS omitted coefficient shadow is not zero"));
        }

        for column in 0..dimension {
            let value = shadow.covariance[row * dimension + column];
            let transposed = shadow.covariance[column * dimension + row];
            if !value.is_finite()
                || !transposed.is_finite()
                || (value - transposed).abs()
                    > 1e-10_f64 * value.abs().max(transposed.abs()).max(1.0)
            {
                return Err(invalid("OLS covariance shadow is nonfinite or asymmetric"));
            }
            let expected_marker = f64::from(reported && row == column);
            if shadow.omission_v_marker[row * dimension + column] != expected_marker {
                return Err(invalid("OLS omission covariance marker is invalid"));
            }
            if !reported && (value != 0.0 || transposed != 0.0) {
                return Err(invalid("OLS omitted covariance row is not zero"));
            }
        }
        if shadow.covariance[row * dimension + row] < 0.0 {
            return Err(invalid("OLS covariance shadow has a negative diagonal"));
        }
    }
    if reported_coordinates != shadow.covariance_dimension {
        return Err(invalid(
            "OLS covariance dimension disagrees with coefficient statuses",
        ));
    }
    Ok(())
}

fn validate_defined_value_table(values: &[f64], label: &str) -> Result<()> {
    if values.len() % 2 != 0 {
        return Err(invalid(format!("{label} has an invalid width")));
    }
    for row in values.chunks_exact(2) {
        if !matches!(row[0], 0.0 | 1.0) || !row[1].is_finite() || (row[0] == 0.0 && row[1] != 0.0) {
            return Err(invalid(format!("{label} has an invalid defined/value row")));
        }
    }
    Ok(())
}

fn validate_shadow_diagnostics(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
) -> Result<()> {
    for row in [0_usize, 1, 2, 23, 31] {
        if shadow.certificates[row * 2] != 1.0 || shadow.certificates[row * 2 + 1] != 1.0 {
            return Err(invalid("OLS mandatory native certificate did not pass"));
        }
    }
    if shadow.certificates[30 * 2] != 1.0 || !shadow.certificates[30 * 2 + 1].is_finite() {
        return Err(invalid("OLS covariance determinant certificate is invalid"));
    }
    for row in shadow.solver.chunks_exact(SOLVER_COLUMNS) {
        if row.iter().any(|value| !value.is_finite())
            || !matches!(row[0], 1.0 | 2.0 | 3.0 | 4.0 | 5.0 | 6.0 | 7.0)
            || !matches!(row[1], 1.0 | 2.0 | 3.0 | 4.0 | 5.0 | 6.0 | 7.0)
            || exact_usize(row[2], "OLS solver iteration count").is_err()
            || !matches!(row[3], 1.0 | 2.0 | 3.0 | 4.0)
            || !matches!(row[10], 0.0 | 1.0)
            || exact_usize(row[11], "OLS solver warning count").is_err()
            || !matches!(row[12], 0.0 | 1.0 | 2.0 | 3.0 | 4.0)
            || !matches!(row[13], 0.0 | 1.0 | 2.0 | 3.0 | 4.0)
            || exact_usize(row[14], "OLS solver pilot iteration count").is_err()
            || row[15] < 0.0
        {
            return Err(invalid("OLS solver shadow metadata is invalid"));
        }
    }
    for row in shadow.fixed_effects.chunks_exact(FE_COLUMNS) {
        validate_shadow_fixed_effect_row(row)?;
    }
    if shadow.dof[0] != 1.0
        || shadow.dof[12] != 1.0
        || shadow.dof[14] != 1.0
        || shadow.dof[18] != 1.0
        || shadow.dof[1] <= 0.0
        || shadow.dof[19] == 0.0
    {
        return Err(invalid("OLS required DoF shadow fields are invalid"));
    }
    if action.vce != OlsVceKind::Clustered {
        if shadow.clusters.iter().any(|value| *value != 0.0)
            || shadow.subsets.iter().any(|value| *value != 0.0)
        {
            return Err(invalid("nonclustered OLS has nonempty cluster shadows"));
        }
    } else {
        let maximum_mask = 1_usize
            .checked_shl(
                u32::try_from(action.clusters.len())
                    .map_err(|_| allocation("OLS cluster count exceeds u32"))?,
            )
            .ok_or_else(|| allocation("OLS cluster subset mask overflows"))?
            - 1;
        for (index, row) in shadow.clusters.chunks_exact(CLUSTER_COLUMNS).enumerate() {
            if exact_usize(row[0], "OLS cluster identity")? != index + 1
                || exact_usize(row[1], "OLS cluster arity").is_err()
                || row[1] < 1.0
                || exact_usize(row[2], "OLS cluster group count").is_err()
                || row[2] < 2.0
                || !matches!(row[3], 0.0 | 1.0)
            {
                return Err(invalid("OLS cluster shadow metadata is invalid"));
            }
        }
        for row in shadow.subsets.chunks_exact(SUBSET_COLUMNS) {
            let mask = exact_usize(row[0], "OLS cluster subset mask")?;
            let dimensions = exact_usize(row[1], "OLS cluster subset dimension count")?;
            if mask == 0
                || mask > maximum_mask
                || dimensions == 0
                || dimensions > action.clusters.len()
                || !matches!(row[2], -1.0 | 1.0)
                || exact_usize(row[3], "OLS cluster subset group count").is_err()
                || row[3] < 1.0
            {
                return Err(invalid("OLS cluster-subset shadow metadata is invalid"));
            }
        }
    }
    Ok(())
}

fn validate_shadow_fixed_effect_row(row: &[f64]) -> Result<()> {
    if row.len() != FE_COLUMNS {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    }
    let Ok(identity) = exact_usize(row[0], "OLS FE identity") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(term) = exact_usize(row[1], "OLS FE term identity") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(source) = exact_usize(row[2], "OLS FE source identity") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(categories) = exact_usize(row[3], "OLS FE category count") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(redundant) = exact_usize(row[4], "OLS FE redundancy") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(coefficients) = exact_usize(row[5], "OLS FE coefficient count") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let identity_is_valid = match identity {
        // One categorical source is represented directly.
        1 => term > 0 && source > 0,
        // The implicit no-absorb constant has no term or source identity.
        2 => term == 0 && source == 0,
        // A categorical tuple retains the absorbed-term identity, while the
        // single source cell is deliberately zero because no one source is
        // authoritative for the interaction partition.
        3 => term > 0 && source == 0,
        // The guarded continuous-slope row retains its term identity and the
        // slope-source identity in the source cell.
        4 => term > 0 && source > 0,
        _ => false,
    };
    if !identity_is_valid
        || categories == 0
        || redundant > categories
        || coefficients != categories - redundant
        || !matches!(row[6], 0.0 | 1.0)
        || !matches!(row[7], 0.0 | 1.0)
    {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    }
    Ok(())
}

fn store_shadow(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
    derived: &DerivedObservationOutputs,
    runtime: &[f64],
    memory_report: &MemoryReport,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    let mut retained_cursor = 0_usize;
    for (zero_based, reason) in shadow.drop_reasons.iter().copied().enumerate() {
        let observation = zero_based + 1;
        let retained = reason == fereg_core::DropReason::Retained;
        if !retained && reason != fereg_core::DropReason::ExcludedByIfIn {
            spi.store_numeric(action.outputs.sample, observation, 0.0)?;
        }
        if retained {
            debug_assert_eq!(shadow.retained_rows[retained_cursor], zero_based);
            if let Some(position) = action.outputs.residual {
                spi.store_numeric(position, observation, shadow.residuals[retained_cursor])?;
            }
            for (position, values) in [
                (action.outputs.xb, derived.xb.as_deref()),
                (action.outputs.xbd, derived.xbd.as_deref()),
                (action.outputs.absorbed, derived.absorbed.as_deref()),
            ] {
                if let (Some(position), Some(values)) = (position, values) {
                    spi.store_numeric(position, observation, values[retained_cursor])?;
                }
            }
            for (position, values) in action
                .outputs
                .saved_channels
                .iter()
                .zip(&derived.saved_channels)
            {
                if let (Some(position), Some(values)) = (*position, values.as_ref()) {
                    spi.store_numeric(position, observation, values[retained_cursor])?;
                }
            }
            retained_cursor += 1;
        }
        if let Some(position) = action.outputs.drop_reason {
            spi.store_numeric(position, observation, f64::from(reason.code()))?;
        }
    }
    store_matrix(spi, &action.matrices.b, 1, shadow.b.len(), &shadow.b)?;
    let dimension = shadow.coefficient_count.max(1);
    store_matrix(
        spi,
        &action.matrices.covariance,
        dimension,
        dimension,
        &shadow.covariance,
    )?;
    store_matrix(
        spi,
        &action.matrices.coefficient_status,
        dimension,
        COEFFICIENT_STATUS_COLUMNS,
        &shadow.coefficient_status,
    )?;
    store_matrix(
        spi,
        &action.matrices.omission_b_marker,
        1,
        dimension,
        &shadow.omission_b_marker,
    )?;
    store_matrix(
        spi,
        &action.matrices.omission_v_marker,
        dimension,
        dimension,
        &shadow.omission_v_marker,
    )?;
    store_matrix(spi, &action.matrices.fit, FIT_ROWS, 2, &shadow.fit)?;
    store_matrix(spi, &action.matrices.dof, DOF_ROWS, 2, &shadow.dof)?;
    store_matrix(
        spi,
        &action.matrices.solver,
        SOLVER_ROWS,
        SOLVER_COLUMNS,
        &shadow.solver,
    )?;
    store_matrix(
        spi,
        &action.matrices.fixed_effects,
        shadow.fixed_effect_rows,
        FE_COLUMNS,
        &shadow.fixed_effects,
    )?;
    store_matrix(
        spi,
        &action.matrices.clusters,
        shadow.cluster_rows,
        CLUSTER_COLUMNS,
        &shadow.clusters,
    )?;
    store_matrix(
        spi,
        &action.matrices.subsets,
        shadow.subset_rows,
        SUBSET_COLUMNS,
        &shadow.subsets,
    )?;
    store_matrix(
        spi,
        &action.matrices.certificates,
        CERTIFICATE_ROWS,
        2,
        &shadow.certificates,
    )?;
    store_matrix(spi, &action.matrices.runtime, RUNTIME_ROWS, 2, runtime)?;
    store_matrix(spi, &action.matrices.memory_report, MEMORY_ROWS, 2, &memory_report.0)?;
    for column in 1..RECEIPT_COMPLETE_COLUMN {
        spi.store_matrix(
            &action.matrices.receipt,
            1,
            column,
            shadow.receipt[column - 1],
        )?;
    }
    // This must remain the final operation: no allocation, polling, display,
    // computation, or store is permitted after the completion cell succeeds.
    spi.store_matrix(&action.matrices.receipt, 1, RECEIPT_COMPLETE_COLUMN, 1.0)
}

fn store_matrix(
    spi: &mut dyn SpiApi,
    name: &PreparedMatrixStoreName,
    rows: usize,
    columns: usize,
    values: &[f64],
) -> Result<()> {
    debug_assert_eq!(values.len(), rows * columns);
    for row in 1..=rows {
        for column in 1..=columns {
            spi.store_matrix(name, row, column, values[(row - 1) * columns + column - 1])?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_layout(
    request: &OlsRequestV1,
    outputs: &ObservationOutputs,
    matrices: &ResultMatrices,
    coefficient_count: usize,
    fe_rows: usize,
    cluster_count: usize,
    spi: &mut dyn SpiApi,
    input_matrix_names: &[&String],
) -> Result<PreparedResultMatrices> {
    let nvars = spi.nvars()?;
    let output_count = 1
        + usize::from(outputs.residual.is_some())
        + usize::from(outputs.drop_reason.is_some())
        + usize::from(outputs.xb.is_some())
        + usize::from(outputs.xbd.is_some())
        + usize::from(outputs.absorbed.is_some())
        + outputs
            .saved_channels
            .iter()
            .filter(|value| value.is_some())
            .count();
    let expected = request
        .bindings
        .len()
        .checked_add(output_count)
        .ok_or_else(|| allocation("OLS plugin varlist width overflows"))?;
    if nvars != expected {
        return Err(invalid(format!(
            "plugin varlist has {nvars} variables, expected {expected}"
        )));
    }
    let mut positions = BTreeSet::new();
    for binding in &request.bindings {
        match binding.role {
            OlsInputRole::NumericSource(source) if !source_requires_numeric(request, source) => {
                validate_categorical_position(
                    binding.stata_var_position,
                    nvars,
                    &mut positions,
                    spi,
                )?;
            }
            _ => {
                validate_numeric_position(binding.stata_var_position, nvars, &mut positions, spi)?;
            }
        }
    }
    validate_numeric_position(outputs.sample, nvars, &mut positions, spi)?;
    if let Some(position) = outputs.residual {
        validate_numeric_position(position, nvars, &mut positions, spi)?;
    }
    if let Some(position) = outputs.drop_reason {
        validate_numeric_position(position, nvars, &mut positions, spi)?;
    }
    for position in [outputs.xb, outputs.xbd, outputs.absorbed]
        .into_iter()
        .flatten()
        .chain(outputs.saved_channels.iter().copied().flatten())
    {
        validate_numeric_position(position, nvars, &mut positions, spi)?;
    }
    let nvars_u32 = u32::try_from(nvars).map_err(|_| invalid("plugin varlist exceeds u32"))?;
    if positions.len() != nvars || positions.iter().copied().ne(1..=nvars_u32) {
        return Err(invalid(
            "OLS input/output bindings must cover the plugin varlist exactly",
        ));
    }
    let matrix_dimension = coefficient_count.max(1);
    let subset_count = if cluster_count == 0 {
        1
    } else {
        (1_usize
            .checked_shl(
                u32::try_from(cluster_count)
                    .map_err(|_| allocation("cluster count exceeds u32"))?,
            )
            .ok_or_else(|| allocation("cluster subset count overflows"))?
            - 1)
        .max(1)
    };
    let cluster_rows = cluster_count.max(1);
    let prepared_matrices = PreparedResultMatrices {
        b: PreparedMatrixStoreName::try_new(&matrices.b, 1, matrix_dimension)?,
        covariance: PreparedMatrixStoreName::try_new(
            &matrices.covariance,
            matrix_dimension,
            matrix_dimension,
        )?,
        coefficient_status: PreparedMatrixStoreName::try_new(
            &matrices.coefficient_status,
            matrix_dimension,
            COEFFICIENT_STATUS_COLUMNS,
        )?,
        omission_b_marker: PreparedMatrixStoreName::try_new(
            &matrices.omission_b_marker,
            1,
            matrix_dimension,
        )?,
        omission_v_marker: PreparedMatrixStoreName::try_new(
            &matrices.omission_v_marker,
            matrix_dimension,
            matrix_dimension,
        )?,
        fit: PreparedMatrixStoreName::try_new(&matrices.fit, FIT_ROWS, 2)?,
        dof: PreparedMatrixStoreName::try_new(&matrices.dof, DOF_ROWS, 2)?,
        solver: PreparedMatrixStoreName::try_new(&matrices.solver, SOLVER_ROWS, SOLVER_COLUMNS)?,
        fixed_effects: PreparedMatrixStoreName::try_new(
            &matrices.fixed_effects,
            fe_rows,
            FE_COLUMNS,
        )?,
        clusters: PreparedMatrixStoreName::try_new(
            &matrices.clusters,
            cluster_rows,
            CLUSTER_COLUMNS,
        )?,
        subsets: PreparedMatrixStoreName::try_new(&matrices.subsets, subset_count, SUBSET_COLUMNS)?,
        certificates: PreparedMatrixStoreName::try_new(
            &matrices.certificates,
            CERTIFICATE_ROWS,
            2,
        )?,
        memory_report: PreparedMatrixStoreName::try_new(&matrices.memory_report, MEMORY_ROWS, 2)?,
        runtime: PreparedMatrixStoreName::try_new(&matrices.runtime, RUNTIME_ROWS, 2)?,
        receipt: PreparedMatrixStoreName::try_new(&matrices.receipt, 1, RECEIPT_COLUMNS)?,
    };
    let result_matrices = [
        (prepared_matrices.b.as_str(), 1, matrix_dimension),
        (
            prepared_matrices.covariance.as_str(),
            matrix_dimension,
            matrix_dimension,
        ),
        (
            prepared_matrices.coefficient_status.as_str(),
            matrix_dimension,
            COEFFICIENT_STATUS_COLUMNS,
        ),
        (
            prepared_matrices.omission_b_marker.as_str(),
            1,
            matrix_dimension,
        ),
        (
            prepared_matrices.omission_v_marker.as_str(),
            matrix_dimension,
            matrix_dimension,
        ),
        (prepared_matrices.fit.as_str(), FIT_ROWS, 2),
        (prepared_matrices.dof.as_str(), DOF_ROWS, 2),
        (
            prepared_matrices.solver.as_str(),
            SOLVER_ROWS,
            SOLVER_COLUMNS,
        ),
        (
            prepared_matrices.fixed_effects.as_str(),
            fe_rows,
            FE_COLUMNS,
        ),
        (
            prepared_matrices.clusters.as_str(),
            cluster_rows,
            CLUSTER_COLUMNS,
        ),
        (
            prepared_matrices.subsets.as_str(),
            subset_count,
            SUBSET_COLUMNS,
        ),
        (prepared_matrices.certificates.as_str(), CERTIFICATE_ROWS, 2),
        (prepared_matrices.runtime.as_str(), RUNTIME_ROWS, 2),
        (prepared_matrices.memory_report.as_str(), MEMORY_ROWS, 2),
        (prepared_matrices.receipt.as_str(), 1, RECEIPT_COLUMNS),
    ];
    let mut names = BTreeSet::new();
    for name in input_matrix_names {
        if !names.insert(name.as_str()) {
            return Err(invalid("OLS input matrix name is duplicated"));
        }
    }
    for (name, rows, columns) in result_matrices {
        if !names.insert(name) {
            return Err(invalid(
                "OLS result matrix name is duplicated or aliases an input",
            ));
        }
        if spi.matrix_dimensions(name)? != (rows, columns) {
            return Err(invalid(format!(
                "OLS result matrix {name:?} has the wrong dimensions",
            )));
        }
    }
    if spi.matrix_element(
        prepared_matrices.receipt.as_str(),
        1,
        RECEIPT_COMPLETE_COLUMN,
    )? != 0.0
    {
        return Err(invalid(
            "OLS shadow receipt must be incomplete before observation loading",
        ));
    }
    Ok(prepared_matrices)
}

fn validate_numeric_position(
    position: u32,
    nvars: usize,
    positions: &mut BTreeSet<u32>,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    if position == 0
        || usize::try_from(position).map_or(true, |value| value > nvars)
        || !positions.insert(position)
    {
        return Err(invalid("OLS plugin positions must be unique and in range"));
    }
    if spi.variable_is_string(position)? {
        return Err(unsupported(format!(
            "plugin varlist position {position} must be numeric"
        )));
    }
    Ok(())
}

fn validate_categorical_position(
    position: u32,
    nvars: usize,
    positions: &mut BTreeSet<u32>,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    if position == 0
        || usize::try_from(position).map_or(true, |value| value > nvars)
        || !positions.insert(position)
    {
        return Err(invalid("OLS plugin positions must be unique and in range"));
    }
    if spi.variable_is_string(position)? && spi.variable_is_strl(position)? {
        return Err(unsupported(format!(
            "plugin varlist position {position} uses unsupported strL storage"
        )));
    }
    Ok(())
}

fn source_requires_numeric(request: &OlsRequestV1, source: SourceId) -> bool {
    request.model.terms.iter().any(|term| {
        term.components.iter().any(|component| {
            component.source == source && component.kind == ComponentKind::Continuous
        })
    }) || request
        .absorbed_terms
        .iter()
        .any(|term| term.slope_sources.contains(&source))
}

pub(crate) fn read_table(
    spi: &mut dyn SpiApi,
    name: &str,
    declared_count: usize,
    width: usize,
    label: &str,
) -> Result<NumericWireTable> {
    let (rows, columns) = spi.matrix_dimensions(name)?;
    let expected_rows = declared_count.max(1);
    if (rows, columns) != (expected_rows, width) {
        return Err(invalid(format!(
            "{label} matrix {name:?} is {rows}x{columns}, expected {expected_rows}x{width}",
        )));
    }
    let mut values = reserved_empty(rows, "numeric wire table")?;
    for row in 1..=rows {
        let mut output_row = reserved_empty(width, "numeric wire row")?;
        for column in 1..=width {
            push_reserved(
                &mut output_row,
                spi.matrix_element(name, row, column)?,
                "numeric wire row capacity exhausted",
            )?;
        }
        push_reserved(
            &mut values,
            output_row,
            "numeric wire table capacity exhausted",
        )?;
    }
    Ok(NumericWireTable {
        declared_count: declared_count as f64,
        rows: values,
    })
}

pub(crate) fn read_bindings(
    spi: &mut dyn SpiApi,
    name: &str,
    count: usize,
) -> Result<Vec<OlsInputBinding>> {
    if count == 0 {
        return Err(invalid("OLS binding matrix cannot be empty"));
    }
    if spi.matrix_dimensions(name)? != (count, BINDING_ROW_WIDTH) {
        return Err(invalid("OLS binding matrix has the wrong dimensions"));
    }
    let mut bindings = reserved_empty(count, "OLS input bindings")?;
    for row in 1..=count {
        let position = exact_positive_u32(spi.matrix_element(name, row, 1)?, "binding position")?;
        let role = exact_usize(spi.matrix_element(name, row, 2)?, "binding role")?;
        let source = exact_usize(spi.matrix_element(name, row, 3)?, "binding source")?;
        let role = match (role, source) {
            (1, 0) => OlsInputRole::Candidate,
            (2, 0) => OlsInputRole::Outcome,
            (3, source) if source > 0 => OlsInputRole::NumericSource(SourceId::new(
                u32::try_from(source).map_err(|_| invalid("binding source exceeds u32"))?,
            )?),
            (4, 0) => OlsInputRole::AnalyticWeight,
            (5, 0) => OlsInputRole::FrequencyWeight,
            (6, 0) => OlsInputRole::ProbabilityWeight,
            _ => return Err(invalid(format!("invalid OLS binding row {row}"))),
        };
        push_reserved(
            &mut bindings,
            OlsInputBinding {
                stata_var_position: position,
                role,
            },
            "OLS input-binding capacity exhausted",
        )?;
    }
    Ok(bindings)
}

#[derive(Clone, Copy)]
enum InputRoleTag {
    Candidate,
    Outcome,
}

fn role_position(request: &OlsRequestV1, target: InputRoleTag) -> Result<u32> {
    request
        .bindings
        .iter()
        .find_map(|binding| match (target, binding.role) {
            (InputRoleTag::Candidate, OlsInputRole::Candidate)
            | (InputRoleTag::Outcome, OlsInputRole::Outcome) => Some(binding.stata_var_position),
            _ => None,
        })
        .ok_or_else(|| invalid("required OLS input binding is missing"))
}

fn source_position(request: &OlsRequestV1, source: SourceId) -> Result<u32> {
    request
        .bindings
        .iter()
        .find_map(|binding| {
            (binding.role == OlsInputRole::NumericSource(source))
                .then_some(binding.stata_var_position)
        })
        .ok_or_else(|| invalid("OLS numeric source binding is missing"))
}

fn weight_position(request: &OlsRequestV1, role: OlsInputRole) -> Option<u32> {
    request
        .bindings
        .iter()
        .find_map(|binding| (binding.role == role).then_some(binding.stata_var_position))
}

fn empty_loaded_column(n: usize, label: &'static str) -> Result<LoadedNumericColumn> {
    Ok(LoadedNumericColumn {
        values: reserved_filled(n, 0.0, label)?,
        missing: reserved_filled(n, false, label)?,
    })
}

pub(crate) fn decode_registry(value: &str, expected: usize) -> Result<Vec<String>> {
    if expected == 0 {
        if value != "." {
            return Err(invalid("empty registry must use the dot sentinel"));
        }
        return Ok(Vec::new());
    }
    if value.split('|').count() != expected {
        return Err(invalid(
            "registry cardinality does not match its declared count",
        ));
    }
    let mut output = reserved_empty(expected, "length-framed registry")?;
    for piece in value.split('|') {
        let (length, payload) = piece
            .split_once(':')
            .ok_or_else(|| invalid("registry item lacks a length prefix"))?;
        let length = length
            .parse::<usize>()
            .map_err(|_| syntax("registry length must be an unsigned integer"))?;
        if payload.len() != length {
            return Err(invalid("registry byte length is incorrect"));
        }
        let mut text = String::new();
        text.try_reserve_exact(payload.len())
            .map_err(|_| allocation("unable to reserve registry payload"))?;
        text.push_str(payload);
        push_reserved(&mut output, text, "registry capacity exhausted")?;
    }
    Ok(output)
}

fn decode_output_position_registry(value: &str, expected: usize) -> Result<Vec<Option<u32>>> {
    let values = decode_registry(value, expected)?;
    let mut output = reserved_empty(expected, "saved-output position registry")?;
    for value in values {
        let position = value
            .parse::<u32>()
            .map_err(|_| syntax("saved-output position must be an unsigned integer"))?;
        push_reserved(
            &mut output,
            (position != 0).then_some(position),
            "saved-output position registry capacity exhausted",
        )?;
    }
    Ok(output)
}

fn parse_vce(value: &str) -> Result<OlsVceKind> {
    match value.to_ascii_lowercase().as_str() {
        "iid" | "unadjusted" | "ols" => Ok(OlsVceKind::Iid),
        "robust" => Ok(OlsVceKind::Robust),
        "cluster" | "clustered" => Ok(OlsVceKind::Clustered),
        _ => Err(unsupported(format!(
            "unsupported guarded OLS VCE {value:?}"
        ))),
    }
}

fn parse_dof(value: &str) -> Result<PreparedOlsAbsorbedDofPolicy> {
    match value.to_ascii_lowercase().as_str() {
        "none" => Ok(PreparedOlsAbsorbedDofPolicy::None),
        "firstpair" => Ok(PreparedOlsAbsorbedDofPolicy::FirstPair),
        "pairwise" => Ok(PreparedOlsAbsorbedDofPolicy::Pairwise),
        "continuous" => Ok(PreparedOlsAbsorbedDofPolicy::Continuous),
        _ => Err(unsupported(format!(
            "unsupported guarded OLS DoF policy {value:?}"
        ))),
    }
}

fn parse_solver(value: &str) -> Result<FeSolverKind> {
    match value {
        "auto" => Ok(FeSolverKind::Auto),
        "map" => Ok(FeSolverKind::Map),
        "lsmr" => Ok(FeSolverKind::Lsmr),
        "cmg" => Ok(FeSolverKind::Cmg),
        "cg" => Ok(FeSolverKind::Cg),
        "it" | "irons-tuck" => Ok(FeSolverKind::IronsTuck),
        "squarem" => Ok(FeSolverKind::Squarem),
        _ => Err(syntax(
            "solver must be auto, map, lsmr, cmg, cg, it, or squarem",
        )),
    }
}

fn parse_preconditioner(value: &str) -> Result<LsmrPreconditioner> {
    match value {
        "auto" => Ok(LsmrPreconditioner::Auto),
        "off" | "none" => Ok(LsmrPreconditioner::Off),
        "diagonal" => Ok(LsmrPreconditioner::Diagonal),
        "schwarz" | "additive" => Ok(LsmrPreconditioner::Schwarz),
        _ => Err(syntax(
            "preconditioner must be auto, off, diagonal, or schwarz",
        )),
    }
}

pub(crate) fn required<'a>(values: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str> {
    values
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| syntax(format!("missing required field {key}")))
}

pub(crate) fn parse_usize(values: &BTreeMap<String, String>, key: &str) -> Result<usize> {
    required(values, key)?
        .parse::<usize>()
        .map_err(|_| syntax(format!("field {key} must be an unsigned integer")))
}

pub(crate) fn parse_positive_usize(values: &BTreeMap<String, String>, key: &str) -> Result<usize> {
    let value = parse_usize(values, key)?;
    if value == 0 {
        Err(syntax(format!("field {key} must be positive")))
    } else {
        Ok(value)
    }
}

pub(crate) fn parse_positive_f64(values: &BTreeMap<String, String>, key: &str) -> Result<f64> {
    let value = required(values, key)?
        .parse::<f64>()
        .map_err(|_| syntax(format!("field {key} must be numeric")))?;
    if value.is_finite() && value > 0.0 && value < 1.0 {
        Ok(value)
    } else {
        Err(syntax(format!("field {key} must be finite and in (0,1)")))
    }
}

pub(crate) fn imposed_memory_limit_bytes() -> Result<Option<u128>> {
    let Some(value) = std::env::var_os("FEREG_MEMORY_LIMIT_BYTES") else {
        return Ok(instrumentation::system_available_memory_bytes());
    };
    let value = value
        .to_str()
        .ok_or_else(|| syntax("FEREG_MEMORY_LIMIT_BYTES is not UTF-8"))?;
    let bytes = value
        .parse::<u128>()
        .map_err(|_| syntax("FEREG_MEMORY_LIMIT_BYTES must be an unsigned integer"))?;
    if bytes == 0 {
        Err(syntax("FEREG_MEMORY_LIMIT_BYTES must be positive"))
    } else {
        Ok(Some(bytes))
    }
}

pub(crate) fn parse_u16(values: &BTreeMap<String, String>, key: &str) -> Result<u16> {
    required(values, key)?
        .parse::<u16>()
        .map_err(|_| syntax(format!("field {key} must be a u16 integer")))
}

pub(crate) fn parse_positive_u32(values: &BTreeMap<String, String>, key: &str) -> Result<u32> {
    let value = parse_nonnegative_u32(values, key)?;
    if value == 0 {
        Err(syntax(format!("field {key} must be positive")))
    } else {
        Ok(value)
    }
}

pub(crate) fn parse_nonnegative_u32(values: &BTreeMap<String, String>, key: &str) -> Result<u32> {
    required(values, key)?
        .parse::<u32>()
        .map_err(|_| syntax(format!("field {key} must be a u32 integer")))
}

pub(crate) fn parse_bool(values: &BTreeMap<String, String>, key: &str) -> Result<bool> {
    match required(values, key)? {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(syntax(format!("field {key} must be zero or one"))),
    }
}

fn exact_usize(value: f64, label: &str) -> Result<usize> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return Err(invalid(format!(
            "{label} must be an exact nonnegative integer"
        )));
    }
    Ok(value as usize)
}

fn exact_positive_u32(value: f64, label: &str) -> Result<u32> {
    if !value.is_finite() || value < 1.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(invalid(format!("{label} must be an exact positive u32")));
    }
    Ok(value as u32)
}

fn reserved_empty<T>(elements: usize, label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    if output.capacity() < elements {
        return Err(allocation(format!("{label} reservation was undersized")));
    }
    Ok(output)
}

fn reserved_filled<T: Clone>(elements: usize, value: T, label: &'static str) -> Result<Vec<T>> {
    let mut output = reserved_empty(elements, label)?;
    output.resize(elements, value);
    Ok(output)
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn syntax(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Syntax, message)
}
fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}
fn unsupported(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Unsupported, message)
}
fn protocol(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::ProtocolMismatch, message)
}
fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fereg_core::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledModelSpecification, CompiledTermGraph,
        ComponentKind, ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, ModelWireV2,
        RawTermKind, SourceEntry, TermComponent, TermId, TermOrder, TimeSeriesState,
    };
    use fereg_core::protocol::{ACTION_PROTOCOL_VERSION, ADO_VERSION, MODEL_WIRE_VERSION};
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Debug, Clone, PartialEq)]
    enum StoreEvent {
        Numeric(u32, usize, f64),
        Matrix(u8, usize, usize, f64),
    }

    #[derive(Clone)]
    struct FakeSpi {
        local_bundle: Option<Vec<u8>>,
        macro_reads: usize,
        nobs: usize,
        nvars: usize,
        data: BTreeMap<(u32, usize), (f64, bool)>,
        string_data: BTreeMap<(u32, usize), Vec<u8>>,
        matrices: BTreeMap<String, Vec<Vec<f64>>>,
        string_variables: BTreeSet<u32>,
        strl_variables: BTreeSet<u32>,
        numeric_reads: usize,
        numeric_batches: Vec<(usize, usize, Vec<u32>)>,
        store_calls: usize,
        fail_store_at: Option<usize>,
        interrupt_at: Option<usize>,
        interrupt_checks: usize,
        stores: Vec<StoreEvent>,
        store_error: Option<FeregError>,
    }

    impl SpiApi for FakeSpi {
        fn read_estimate_bundle(&mut self, buffer: &mut [u8]) -> Result<()> {
            self.macro_reads += 1;
            let value = self
                .local_bundle
                .as_ref()
                .ok_or_else(|| invalid("fake local request macro is unavailable"))?;
            let copied = value.len().min(buffer.len() - 1);
            buffer[..copied].copy_from_slice(&value[..copied]);
            buffer[copied] = 0;
            Ok(())
        }

        fn nobs(&mut self) -> Result<usize> {
            Ok(self.nobs)
        }

        fn nvars(&mut self) -> Result<usize> {
            Ok(self.nvars)
        }

        fn variable_is_string(&mut self, variable: u32) -> Result<bool> {
            Ok(self.string_variables.contains(&variable))
        }

        fn variable_is_strl(&mut self, variable: u32) -> Result<bool> {
            Ok(self.strl_variables.contains(&variable))
        }

        fn read_numeric(&mut self, variable: u32, observation: usize) -> Result<(f64, bool)> {
            self.numeric_reads += 1;
            self.data
                .get(&(variable, observation))
                .copied()
                .ok_or_else(|| invalid("test numeric cell is absent"))
        }

        fn read_numeric_columns(
            &mut self,
            variables: &[u32],
            first_observation: usize,
            columns: &mut [&mut [f64]],
            missing: &mut [&mut [bool]],
        ) -> Result<()> {
            self.numeric_batches
                .push((first_observation, columns[0].len(), variables.to_vec()));
            for offset in 0..columns[0].len() {
                for ((variable, column), flags) in variables
                    .iter()
                    .zip(columns.iter_mut())
                    .zip(missing.iter_mut())
                {
                    let (value, is_missing) =
                        self.read_numeric(*variable, first_observation + offset)?;
                    column[offset] = value;
                    flags[offset] = is_missing;
                }
            }
            Ok(())
        }

        fn read_string(&mut self, variable: u32, observation: usize) -> Result<Vec<u8>> {
            self.string_data
                .get(&(variable, observation))
                .cloned()
                .ok_or_else(|| invalid("test string cell is absent"))
        }

        fn matrix_dimensions(&mut self, name: &str) -> Result<(usize, usize)> {
            let matrix = self
                .matrices
                .get(name)
                .ok_or_else(|| invalid(format!("test matrix {name:?} is absent")))?;
            Ok((matrix.len(), matrix.first().map_or(0, Vec::len)))
        }

        fn matrix_element(&mut self, name: &str, row: usize, column: usize) -> Result<f64> {
            self.matrices
                .get(name)
                .and_then(|matrix| matrix.get(row.checked_sub(1)?))
                .and_then(|values| values.get(column.checked_sub(1)?))
                .copied()
                .ok_or_else(|| invalid("test matrix read is out of range"))
        }

        fn store_numeric(&mut self, variable: u32, observation: usize, value: f64) -> Result<()> {
            self.before_store()?;
            self.data.insert((variable, observation), (value, false));
            self.stores
                .push(StoreEvent::Numeric(variable, observation, value));
            Ok(())
        }

        fn store_matrix(
            &mut self,
            name: &PreparedMatrixStoreName,
            row: usize,
            column: usize,
            value: f64,
        ) -> Result<()> {
            let name = name.as_str();
            self.before_store()?;
            let destination = self
                .matrices
                .get_mut(name)
                .and_then(|matrix| matrix.get_mut(row.checked_sub(1)?))
                .and_then(|values| values.get_mut(column.checked_sub(1)?))
                .ok_or_else(|| invalid("test matrix store is out of range"))?;
            *destination = value;
            self.stores.push(StoreEvent::Matrix(
                matrix_name_code(name),
                row,
                column,
                value,
            ));
            Ok(())
        }

        fn display(&mut self, _message: &str) -> Result<()> {
            Ok(())
        }

        fn check_interrupted(&mut self) -> Result<()> {
            self.interrupt_checks += 1;
            if self.interrupt_at == Some(self.interrupt_checks) {
                Err(FeregError::new(
                    ErrorCode::Interrupted,
                    "injected precommit interruption",
                ))
            } else {
                Ok(())
            }
        }
    }

    impl FakeSpi {
        fn before_store(&mut self) -> Result<()> {
            self.store_calls += 1;
            if self.fail_store_at == Some(self.store_calls) {
                Err(self
                    .store_error
                    .take()
                    .expect("test store error was prepared before commit"))
            } else {
                Ok(())
            }
        }

        fn receipt_complete(&self) -> f64 {
            self.matrices["RREC"][0][RECEIPT_COMPLETE_COLUMN - 1]
        }
    }

    fn matrix_name_code(name: &str) -> u8 {
        match name {
            "RB" => 1,
            "RV" => 2,
            "RSTATUS" => 3,
            "RMARKB" => 4,
            "RMARKV" => 5,
            "RFIT" => 6,
            "RDOF" => 7,
            "RSOLVER" => 8,
            "RFE" => 9,
            "RCLUSTER" => 10,
            "RSUBSET" => 11,
            "RCERT" => 12,
            "RRUNTIME" => 13,
            "RREC" => 14,
            "RMEMORY" => 15,
            _ => panic!("unprepared test matrix-store name {name:?}"),
        }
    }

    fn continuous_term(source: SourceId) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(1).expect("term id"),
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

    fn categorical_term(index: u32, source: SourceId, level: f64, base: bool) -> ExpandedTerm {
        let raw_operator = if base { "0b" } else { "1" };
        ExpandedTerm {
            id: TermId::new(index).expect("term id"),
            order: TermOrder::new(index).expect("term order"),
            coefficient_slot: CoefficientSlot::new(index).expect("coefficient slot"),
            raw_kind: RawTermKind::Factor,
            whole_column_omitted: base,
            canonical_token: format!("{raw_operator}.x"),
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
                raw_operator: raw_operator.to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }
    }

    fn replace_argument(args: &mut [String], key: &str, replacement: String) {
        let prefix = format!("{key}=");
        args.iter_mut()
            .find(|argument| argument.starts_with(&prefix))
            .unwrap_or_else(|| panic!("missing fixture argument {key}"))
            .clone_from(&replacement);
    }

    fn registry(values: &[String]) -> String {
        if values.is_empty() {
            return ".".to_owned();
        }
        values
            .iter()
            .map(|value| format!("{}:{value}", value.len()))
            .collect::<Vec<_>>()
            .join("|")
    }

    fn bundled(args: &[String]) -> Vec<String> {
        let mut payload = format!("{}:", args.len());
        for argument in args {
            payload.push_str(&argument.len().to_string());
            payload.push(':');
            payload.push_str(argument);
        }
        vec![format!("bundle={payload}")]
    }

    fn zero_matrix(rows: usize, columns: usize) -> Vec<Vec<f64>> {
        vec![vec![0.0; columns]; rows]
    }

    fn coefficient_shadow() -> OlsShadowResult<'static> {
        OlsShadowResult {
            solver_workspace_peak: None,
            solver_expected_heap_peak: None,
            b: vec![2.0, 3.0],
            covariance: vec![4.0, 1.0, 1.0, 5.0],
            coefficient_status: vec![1.0, 1.0, 1.0, 1.0, 2.0, 5.0, 0.0, 2.0],
            omission_b_marker: vec![1.0, 1.0],
            omission_v_marker: vec![1.0, 0.0, 0.0, 1.0],
            fit: Vec::new(),
            dof: Vec::new(),
            solver: Vec::new(),
            fixed_effects: Vec::new(),
            clusters: Vec::new(),
            subsets: Vec::new(),
            certificates: Vec::new(),
            receipt: Vec::new(),
            coefficient_count: 2,
            coefficient_capacity: 2,
            covariance_dimension: 2,
            fixed_effect_rows: 0,
            cluster_rows: 0,
            subset_rows: 0,
            drop_reasons: &[],
            retained_rows: &[],
            residuals: &[],
            execution: fereg_core::PreparedOlsExecutionReport {
                mode: PreparedOlsExecutionMode::Fast,
                poolsize: 10,
                scratch_bytes: 0,
                requested_threads: 1,
                effective_threads: 1,
                deterministic: true,
            },
        }
    }

    #[test]
    fn memory_baseline_and_matrix_are_validated_before_any_store() {
        for replacement in ["stata_alloc_before=-1", "stata_used_before=9007199254740993", "output_memory_matrix=RB"] {
            let (mut args, mut spi) = fixture(OlsVceKind::Iid);
            let key = replacement.split_once('=').unwrap().0;
            *args.iter_mut().find(|arg| arg.starts_with(&format!("{key}="))).unwrap() = replacement.to_owned();
            assert!(estimate_ols_test(&args, &mut spi).is_err(), "{replacement}");
            assert_eq!(spi.store_calls, 0);
            assert_eq!(spi.receipt_complete(), 0.0);
        }
    }

    #[test]
    fn partial_memory_report_is_published_without_claiming_combined_coverage() {
        let (args, mut spi) = fixture(OlsVceKind::Iid);
        estimate_ols_test(&args, &mut spi).unwrap();
        let report = &spi.matrices["RMEMORY"];
        assert_eq!(report[0], vec![1.0, 2.0]);
        assert_eq!(report[1], vec![1.0, 1.0]);
        assert_eq!(report[3], vec![0.0, 0.0]);
        assert_eq!(report[7], vec![0.0, 0.0]);
        assert_eq!(report[8], vec![0.0, 0.0]);
        assert!(report[10][1] > 0.0);
        assert_eq!(spi.receipt_complete(), 1.0);
    }

    fn fixture(vce: OlsVceKind) -> (Vec<String>, FakeSpi) {
        let clustered = vce == OlsVceKind::Clustered;
        let sources = if clustered {
            vec![
                SourceEntry {
                    id: SourceId::new(1).expect("x source"),
                    canonical_name: "x".to_owned(),
                },
                SourceEntry {
                    id: SourceId::new(2).expect("cluster source"),
                    canonical_name: "g".to_owned(),
                },
            ]
        } else {
            vec![SourceEntry {
                id: SourceId::new(1).expect("x source"),
                canonical_name: "x".to_owned(),
            }]
        };
        let wire = ModelWireV2::from_graph(&CompiledTermGraph {
            sources,
            terms: vec![continuous_term(SourceId::new(1).expect("x source"))],
        })
        .expect("valid model wire");
        let binding_count = if clustered { 4 } else { 3 };
        let sample_position = if clustered { 5 } else { 4 };
        let residual_position = sample_position + 1;
        let reason_position = sample_position + 2;
        let mut bindings = vec![
            vec![1.0, 1.0, 0.0],
            vec![2.0, 2.0, 0.0],
            vec![3.0, 3.0, 1.0],
        ];
        if clustered {
            bindings.push(vec![4.0, 3.0, 2.0]);
        }
        let mut matrices = BTreeMap::from([
            ("S".to_owned(), wire.sources.rows),
            ("T".to_owned(), wire.terms.rows),
            ("C".to_owned(), wire.components.rows),
            ("A".to_owned(), wire.absorb_terms.rows),
            ("AC".to_owned(), wire.absorb_components.rows),
            ("B".to_owned(), bindings),
            (
                "CT".to_owned(),
                if clustered {
                    vec![vec![1.0, 1.0]]
                } else {
                    zero_matrix(1, CLUSTER_TERM_ROW_WIDTH)
                },
            ),
            (
                "CC".to_owned(),
                if clustered {
                    vec![vec![1.0, 1.0, 2.0]]
                } else {
                    zero_matrix(1, CLUSTER_COMPONENT_ROW_WIDTH)
                },
            ),
            ("RB".to_owned(), zero_matrix(1, 2)),
            ("RV".to_owned(), zero_matrix(2, 2)),
            (
                "RSTATUS".to_owned(),
                zero_matrix(2, COEFFICIENT_STATUS_COLUMNS),
            ),
            ("RMARKB".to_owned(), zero_matrix(1, 2)),
            ("RMARKV".to_owned(), zero_matrix(2, 2)),
            ("RFIT".to_owned(), zero_matrix(FIT_ROWS, 2)),
            ("RDOF".to_owned(), zero_matrix(DOF_ROWS, 2)),
            (
                "RSOLVER".to_owned(),
                zero_matrix(SOLVER_ROWS, SOLVER_COLUMNS),
            ),
            ("RFE".to_owned(), zero_matrix(1, FE_COLUMNS)),
            ("RCLUSTER".to_owned(), zero_matrix(1, CLUSTER_COLUMNS)),
            ("RSUBSET".to_owned(), zero_matrix(1, SUBSET_COLUMNS)),
            ("RCERT".to_owned(), zero_matrix(CERTIFICATE_ROWS, 2)),
            ("RMEMORY".to_owned(), zero_matrix(MEMORY_ROWS, 2)),
            ("RRUNTIME".to_owned(), zero_matrix(RUNTIME_ROWS, 2)),
            ("RREC".to_owned(), zero_matrix(1, RECEIPT_COLUMNS)),
        ]);
        let mut data = BTreeMap::new();
        for (offset, (y, x, g)) in [
            (1.0, 0.0, 1.0),
            (3.0, 1.0, 1.0),
            (2.0, 2.0, 2.0),
            (7.0, 3.0, 2.0),
            (4.0, 4.0, 3.0),
            (8.0, 5.0, 3.0),
        ]
        .into_iter()
        .enumerate()
        {
            let observation = offset + 1;
            data.insert((1, observation), (1.0, false));
            data.insert((2, observation), (y, false));
            data.insert((3, observation), (x, false));
            if clustered {
                data.insert((4, observation), (g, false));
            }
            data.insert((sample_position, observation), (1.0, false));
        }
        let args = vec![
            format!("ado={ADO_VERSION}"),
            format!("protocol_major={}", ACTION_PROTOCOL_VERSION.major),
            format!("protocol_minor={}", ACTION_PROTOCOL_VERSION.minor),
            "stata=18.0".to_owned(),
            format!("confirm={CONFIRMATION}"),
            format!("plugin={PLUGIN_VERSION}"),
            format!("result_major={}", OLS_RESULT_WIRE_VERSION.major),
            format!("result_minor={}", OLS_RESULT_WIRE_VERSION.minor),
            format!("model_major={}", MODEL_WIRE_VERSION.major),
            format!("model_minor={}", MODEL_WIRE_VERSION.minor),
            "source_matrix=S".to_owned(),
            format!("source_count={}", wire.source_names.len()),
            "term_matrix=T".to_owned(),
            "term_count=1".to_owned(),
            "component_matrix=C".to_owned(),
            "component_count=1".to_owned(),
            format!("source_names={}", registry(&wire.source_names)),
            format!("canonical_tokens={}", registry(&wire.canonical_tokens)),
            format!(
                "raw_component_operators={}",
                registry(&wire.raw_component_operators)
            ),
            format!(
                "raw_time_series_operators={}",
                registry(&wire.raw_time_series_operators)
            ),
            "absorb_term_matrix=A".to_owned(),
            "absorb_term_count=0".to_owned(),
            "absorb_component_matrix=AC".to_owned(),
            "absorb_component_count=0".to_owned(),
            "absorb_labels=.".to_owned(),
            "absorb_saved_names=.".to_owned(),
            "cluster_term_matrix=CT".to_owned(),
            format!("cluster_term_count={}", usize::from(clustered)),
            "cluster_component_matrix=CC".to_owned(),
            format!("cluster_component_count={}", usize::from(clustered)),
            if clustered {
                "cluster_labels=1:g".to_owned()
            } else {
                "cluster_labels=.".to_owned()
            },
            "binding_matrix=B".to_owned(),
            format!("binding_count={binding_count}"),
            "include_intercept=1".to_owned(),
            "candidate_all=0".to_owned(),
            "input_complete=0".to_owned(),
            "keep_singletons=0".to_owned(),
            format!(
                "vce={}",
                match vce {
                    OlsVceKind::Iid => "iid",
                    OlsVceKind::Robust => "robust",
                    OlsVceKind::Clustered => "cluster",
                }
            ),
            "dof=pairwise".to_owned(),
            format!("account_cluster_nesting={}", usize::from(clustered)),
            "memory=fast".to_owned(),
            "solver=auto".to_owned(),
            "preconditioner=auto".to_owned(),
            "poolsize=10".to_owned(),
            "threads=1".to_owned(),
            "deterministic=1".to_owned(),
            "tolerance=1e-8".to_owned(),
            "iterate=16000".to_owned(),
            format!("output_sample={sample_position}"),
            format!("output_residual={residual_position}"),
            format!("output_drop_reason={reason_position}"),
            "output_xb=0".to_owned(),
            "output_xbd=0".to_owned(),
            "output_absorbed=0".to_owned(),
            "output_saved_count=0".to_owned(),
            "output_saved_channels=.".to_owned(),
            "output_b_matrix=RB".to_owned(),
            "output_v_matrix=RV".to_owned(),
            "output_status_matrix=RSTATUS".to_owned(),
            "output_marker_b_matrix=RMARKB".to_owned(),
            "output_marker_v_matrix=RMARKV".to_owned(),
            "output_fit_matrix=RFIT".to_owned(),
            "output_dof_matrix=RDOF".to_owned(),
            "output_solver_matrix=RSOLVER".to_owned(),
            "output_fe_matrix=RFE".to_owned(),
            "output_cluster_matrix=RCLUSTER".to_owned(),
            "output_subset_matrix=RSUBSET".to_owned(),
            "output_certificate_matrix=RCERT".to_owned(),
            "output_runtime_matrix=RRUNTIME".to_owned(),
            "output_memory_matrix=RMEMORY".to_owned(),
            "stata_used_entry=.".to_owned(),
            "stata_alloc_entry=.".to_owned(),
            "stata_used_before=.".to_owned(),
            "stata_alloc_before=.".to_owned(),
            "output_receipt_matrix=RREC".to_owned(),
        ];
        (
            args,
            FakeSpi {
                local_bundle: None,
                macro_reads: 0,
                nobs: 6,
                nvars: binding_count + 3,
                data,
                string_data: BTreeMap::new(),
                matrices: std::mem::take(&mut matrices),
                string_variables: BTreeSet::new(),
                strl_variables: BTreeSet::new(),
                numeric_reads: 0,
                numeric_batches: Vec::new(),
                store_calls: 0,
                fail_store_at: None,
                interrupt_at: None,
                interrupt_checks: 0,
                // The allocation-sensitive fake path must not grow this log
                // after the first output store.
                stores: Vec::with_capacity(512),
                store_error: Some(FeregError::new(
                    ErrorCode::Internal,
                    "injected SPI output-store failure",
                )),
            },
        )
    }

    fn saved_output_fixture() -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = fixture(OlsVceKind::Iid);
        let x_source = SourceEntry {
            id: SourceId::new(1).expect("x source"),
            canonical_name: "x".to_owned(),
        };
        let group_source = SourceEntry {
            id: SourceId::new(2).expect("group source"),
            canonical_name: "g".to_owned(),
        };
        let wire = ModelWireV2::from_specification(&CompiledModelSpecification {
            model: CompiledTermGraph {
                sources: vec![x_source.clone(), group_source.clone()],
                terms: vec![continuous_term(x_source.id)],
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorbed term"),
                intercept: true,
                categorical_sources: vec![group_source.id],
                slope_sources: Vec::new(),
                label: "g".to_owned(),
                saved_name: "fe".to_owned(),
            }],
        })
        .expect("saved-output wire");
        for (name, matrix) in [
            ("S", wire.sources.rows),
            ("T", wire.terms.rows),
            ("C", wire.components.rows),
            ("A", wire.absorb_terms.rows),
            ("AC", wire.absorb_components.rows),
        ] {
            spi.matrices.insert(name.to_owned(), matrix);
        }
        spi.matrices.insert(
            "B".to_owned(),
            vec![
                vec![1.0, 1.0, 0.0],
                vec![2.0, 2.0, 0.0],
                vec![3.0, 3.0, 1.0],
                vec![4.0, 3.0, 2.0],
            ],
        );
        for (observation, group) in [1.0, 1.0, 2.0, 2.0, 3.0, 3.0].into_iter().enumerate() {
            spi.data.insert((4, observation + 1), (group, false));
            spi.data.insert((5, observation + 1), (1.0, false));
        }
        spi.nvars = 11;

        replace_argument(&mut args, "source_count", "source_count=2".to_owned());
        replace_argument(
            &mut args,
            "source_names",
            format!("source_names={}", registry(&wire.source_names)),
        );
        replace_argument(
            &mut args,
            "absorb_term_count",
            "absorb_term_count=1".to_owned(),
        );
        replace_argument(
            &mut args,
            "absorb_component_count",
            "absorb_component_count=1".to_owned(),
        );
        replace_argument(
            &mut args,
            "absorb_labels",
            format!("absorb_labels={}", registry(&wire.absorb_labels)),
        );
        replace_argument(
            &mut args,
            "absorb_saved_names",
            format!("absorb_saved_names={}", registry(&wire.absorb_saved_names)),
        );
        replace_argument(&mut args, "binding_count", "binding_count=4".to_owned());
        replace_argument(&mut args, "output_sample", "output_sample=5".to_owned());
        replace_argument(&mut args, "output_residual", "output_residual=6".to_owned());
        replace_argument(
            &mut args,
            "output_drop_reason",
            "output_drop_reason=7".to_owned(),
        );
        replace_argument(&mut args, "output_xb", "output_xb=8".to_owned());
        replace_argument(&mut args, "output_xbd", "output_xbd=9".to_owned());
        replace_argument(
            &mut args,
            "output_absorbed",
            "output_absorbed=10".to_owned(),
        );
        replace_argument(
            &mut args,
            "output_saved_count",
            "output_saved_count=1".to_owned(),
        );
        replace_argument(
            &mut args,
            "output_saved_channels",
            "output_saved_channels=2:11".to_owned(),
        );
        (args, spi)
    }

    fn stored_numeric(spi: &FakeSpi, variable: u32, observation: usize) -> f64 {
        spi.stores
            .iter()
            .find_map(|event| match event {
                StoreEvent::Numeric(found_variable, found_observation, value)
                    if (*found_variable, *found_observation) == (variable, observation) =>
                {
                    Some(*value)
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                spi.data
                    .get(&(variable, observation))
                    .map(|(value, _)| *value)
                    .unwrap_or_else(|| {
                        panic!(
                            "missing stored value for variable {variable}, observation {observation}"
                        )
                    })
            })
    }

    fn factor_scatter_fixture() -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = fixture(OlsVceKind::Robust);
        let source = SourceEntry {
            id: SourceId::new(1).expect("factor source"),
            canonical_name: "x".to_owned(),
        };
        let wire = ModelWireV2::from_graph(&CompiledTermGraph {
            sources: vec![source.clone()],
            terms: vec![
                categorical_term(1, source.id, 0.0, true),
                categorical_term(2, source.id, 1.0, false),
            ],
        })
        .expect("factor model wire");
        spi.matrices.insert("T".to_owned(), wire.terms.rows);
        spi.matrices.insert("C".to_owned(), wire.components.rows);
        spi.matrices.insert("RB".to_owned(), zero_matrix(1, 3));
        spi.matrices.insert("RV".to_owned(), zero_matrix(3, 3));
        spi.matrices.insert(
            "RSTATUS".to_owned(),
            zero_matrix(3, COEFFICIENT_STATUS_COLUMNS),
        );
        spi.matrices.insert("RMARKB".to_owned(), zero_matrix(1, 3));
        spi.matrices.insert("RMARKV".to_owned(), zero_matrix(3, 3));
        for observation in 1..=6 {
            spi.data.insert(
                (3, observation),
                (if observation % 2 == 0 { 0.0 } else { 1.0 }, false),
            );
        }
        replace_argument(&mut args, "term_count", "term_count=2".to_owned());
        replace_argument(&mut args, "component_count", "component_count=2".to_owned());
        replace_argument(
            &mut args,
            "canonical_tokens",
            format!("canonical_tokens={}", registry(&wire.canonical_tokens)),
        );
        replace_argument(
            &mut args,
            "raw_component_operators",
            format!(
                "raw_component_operators={}",
                registry(&wire.raw_component_operators)
            ),
        );
        replace_argument(
            &mut args,
            "raw_time_series_operators",
            format!(
                "raw_time_series_operators={}",
                registry(&wire.raw_time_series_operators)
            ),
        );
        (args, spi)
    }

    #[test]
    #[cfg(feature = "ffi")]
    fn prepared_owner_accounting_matches_independent_allocator_deallocation() {
        use fereg_core::data::WeightKind;
        let _guard = instrumentation::lock_action_observation();
        fn check<T>(owner: T, bytes: u128) {
            let before = crate::ffi::allocation_usage();
            drop(owner);
            let after = crate::ffi::allocation_usage();
            assert_eq!(u128::from(before.current_bytes - after.current_bytes), bytes);
        }
        for complex in [false, true] {
            for clustered in [false, true] {
                for weight in [None, Some(WeightKind::Analytic), Some(WeightKind::Frequency), Some(WeightKind::Probability)] {
                    let (args, mut spi) = saved_output_fixture();
                    let mut action = EstimateActionSpec::parse(&args).unwrap()
                        .decode_with_memory_limit(&mut spi, || Ok(None)).unwrap();
                    let mut loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
                    action.request.keep_singletons = true;
                    action.request.absorbed_terms[0].saved_name.clear();
                    if complex {
                        action.request.absorbed_terms[0].categorical_sources.push(SourceId::new(1).unwrap());
                        action.request.absorbed_terms[0].slope_sources.push(SourceId::new(1).unwrap());
                    }
                    let mut repeated = action.request.absorbed_terms[0].clone();
                    repeated.id = AbsorbTermId::new(2).unwrap();
                    repeated.label = "repeated".to_owned();
                    action.request.absorbed_terms.push(repeated);
                    if let Some(kind) = weight {
                        let column = LoadedNumericColumn { values: vec![2.0; 6], missing: vec![false; 6] };
                        let role = match kind {
                            WeightKind::Analytic => { loaded.analytic_weight = Some(column); OlsInputRole::AnalyticWeight },
                            WeightKind::Frequency => { loaded.frequency_weight = Some(column); OlsInputRole::FrequencyWeight },
                            WeightKind::Probability => { loaded.probability_weight = Some(column); OlsInputRole::ProbabilityWeight },
                            WeightKind::Importance => unreachable!("unsupported fixture weight"),
                        };
                        action.request.bindings.push(OlsInputBinding { stata_var_position: 12, role });
                    }
                    // Retained capacities deliberately exceed logical lengths.
                    loaded.outcome.values.reserve(100);
                    if clustered {
                        let request = OlsClusteredRequestV1 { ols: action.request, clusters: vec![
                            OlsClusterTermV1 { label: "simple".to_owned(), sources: vec![SourceId::new(2).unwrap()] },
                            OlsClusterTermV1 { label: "interaction".to_owned(), sources: vec![SourceId::new(1).unwrap(), SourceId::new(2).unwrap()] },
                        ] };
                        if complex {
                            let owner = prepare_categorical_ols_clustered_data_owned(&request, loaded).unwrap();
                            let before = crate::ffi::allocation_usage();
                            let bytes = owner.ols().owned_buffer_bytes() + owner.clusters().owned_buffer_bytes();
                            assert_eq!(crate::ffi::allocation_usage().events, before.events);
                            check(owner, bytes);
                        } else {
                            let owner = prepare_ols_clustered_data_owned(&request, loaded).unwrap();
                            let bytes = owner.ols().owned_buffer_bytes() + owner.clusters().owned_buffer_bytes();
                            check(owner, bytes);
                        }
                    } else if complex {
                        let owner = prepare_categorical_ols_data_owned(&action.request, loaded).unwrap();
                        let bytes = owner.owned_buffer_bytes();
                        check(owner, bytes);
                    } else {
                        let mut owner = prepare_ols_data_owned(&action.request, loaded).unwrap();
                        owner.sample.outcome.reserve(101);
                        owner.model.columns.reserve(17);
                        owner.categorical_sources[0].encoded.levels.reserve(15);
                        let before = crate::ffi::allocation_usage();
                        let bytes = owner.owned_buffer_bytes();
                        assert_eq!(crate::ffi::allocation_usage().events, before.events);
                        check(owner, bytes);
                    }
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "ffi")]
    fn inference_owner_accounting_matches_allocator_in_all_six_routes() {
        let _guard = instrumentation::lock_action_observation();
        fn check<T>(owner: T, bytes: u128) {
            let before = crate::ffi::allocation_usage();
            drop(owner);
            let after = crate::ffi::allocation_usage();
            assert_eq!(u128::from(before.current_bytes - after.current_bytes), bytes);
        }
        for mode in [PreparedOlsExecutionMode::Fast, PreparedOlsExecutionMode::Low] {
            let (args, mut spi) = saved_output_fixture();
            let mut action = EstimateActionSpec::parse(&args).unwrap()
                .decode_with_memory_limit(&mut spi, || Ok(None)).unwrap();
            action.fit.execution.mode = mode;
            let loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
            action.request.absorbed_terms[0].saved_name.clear();
            let clustered = OlsClusteredRequestV1 {
                ols: action.request.clone(),
                clusters: vec![OlsClusterTermV1 { label: "g".to_owned(), sources: vec![SourceId::new(2).unwrap()] }],
            };
            let iid = PreparedOlsUnadjustedConfig { fit: action.fit, ..Default::default() };
            let robust = PreparedOlsRobustConfig { fit: action.fit, ..Default::default() };
            let cluster = PreparedOlsClusteredConfig { fit: action.fit, ..Default::default() };
            let prepared = prepare_ols_data_owned(&action.request, loaded.clone()).unwrap();
            let inference = fit_prepared_ols_unadjusted_map(&prepared, iid).unwrap();
            let bytes = inference.owned_buffer_bytes();
            check(inference, bytes);
            let inference = fit_prepared_ols_robust_map(&prepared, robust).unwrap();
            let bytes = inference.owned_buffer_bytes();
            check(inference, bytes);
            let prepared = prepare_ols_clustered_data_owned(&clustered, loaded.clone()).unwrap();
            let inference = fit_prepared_ols_clustered_map(&prepared, cluster).unwrap();
            let bytes = inference.owned_buffer_bytes();
            check(inference, bytes);
            let prepared = prepare_categorical_ols_data_owned(&action.request, loaded.clone()).unwrap();
            let inference = fit_prepared_categorical_ols_unadjusted_map(&prepared, iid).unwrap();
            let bytes = inference.owned_buffer_bytes();
            check(inference, bytes);
            let inference = fit_prepared_categorical_ols_robust_map(&prepared, robust).unwrap();
            let bytes = inference.owned_buffer_bytes();
            check(inference, bytes);
            let prepared = prepare_categorical_ols_clustered_data_owned(&clustered, loaded).unwrap();
            let inference = fit_prepared_categorical_ols_clustered_map(&prepared, cluster).unwrap();
            let bytes = inference.owned_buffer_bytes();
            check(inference, bytes);
        }
    }

    #[test]
    #[cfg(feature = "ffi")]
    fn saved_slope_recovery_workspace_bounds_observed_allocator_peak() {
        let _guard = instrumentation::lock_action_observation();
        let (args, mut spi) = saved_output_fixture();
        let mut action = EstimateActionSpec::parse(&args).unwrap()
            .decode_with_memory_limit(&mut spi, || Ok(None)).unwrap();
        action.request.keep_singletons = true;
        action.request.absorbed_terms[0].slope_sources.push(SourceId::new(1).unwrap());
        let loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
        let target = loaded.numeric_sources[0].1.values.iter()
            .zip(&loaded.numeric_sources[1].1.values)
            .map(|(slope, group)| 2.0 * slope + 3.0 * group).collect::<Vec<_>>();
        let prepared = prepare_categorical_ols_data_owned(&action.request, loaded).unwrap();
        for workers in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(workers).build().unwrap();
            pool.install(|| {
                for mask in [[true, true], [false, true]] {
                    let run = || prepared.recover_absorbed_effects_fast(
                        &target, &mask, None, false, true,
                        LsmrConfig::default(), MapConfig { tolerance: 1e-12, ..Default::default() }, None,
                    ).unwrap();
                    // Worker construction belongs to its separately declared
                    // runtime scope. The recovery envelope covers each live
                    // numerical attempt, including pinned within setup.
                    drop(run());
                    for _ in 0..3 {
                        let start = instrumentation::begin_observation();
                        let recovered = run();
                        let usage = crate::ffi::allocation_usage();
                        let observed = usage.peak_bytes - start.allocation_current_bytes;
                        let modeled = recovered.solver_workspace_peak().bytes().unwrap();
                        assert!(observed <= modeled, "slope recovery peak {observed} exceeds {modeled}");
                        for (actual, expected) in recovered.absorbed_sum().iter().zip(&target) {
                            assert!((actual-expected).abs() < 1e-10);
                        }
                    }
                }
            });
        }
    }

    #[test]
    #[cfg(feature = "ffi")]
    fn numeric_dictionary_growth_stays_inside_construction_envelope() {
        let _guard = instrumentation::lock_action_observation();
        for n in [1, 7, 257, 4097, 8193, 65537] {
            for levels in [1, 31, n] {
                let values: Vec<f64> = (0..n).map(|i| (i % levels) as f64 * 100000.5 + 0.25).collect();
                let start = instrumentation::begin_observation();
                let encoded = fereg_core::factorize::factorize_f64(&values).unwrap();
                let peak = crate::ffi::allocation_usage().peak_bytes - start.allocation_current_bytes;
                assert!(u128::from(peak) <= n as u128 * 96 + 4096, "dictionary {n}/{levels}: {peak}");
                for (i, code) in encoded.codes.iter().enumerate() {
                    assert_eq!(encoded.levels[*code as usize], values[i]);
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "ffi")]
    fn construction_and_transient_frame_peaks_bound_independent_allocations() {
        let _guard = instrumentation::lock_action_observation();
        let team = ThreadTeam::new(ThreadConfig::from_requested(4, false).unwrap());
        team.install(|| ()).unwrap();
        for rows in [257, 8193] {
            for complex in [false, true] {
                for vce in [OlsVceKind::Iid, OlsVceKind::Robust, OlsVceKind::Clustered] {
                    let (args, mut spi) = saved_output_fixture();
                    let mut action = EstimateActionSpec::parse(&args).unwrap()
                        .decode_with_memory_limit(&mut spi, || Ok(None)).unwrap();
                    action.request.keep_singletons = true;
                    action.request.absorbed_terms[0].saved_name.clear();
                    if complex {
                        action.request.absorbed_terms[0].slope_sources.push(SourceId::new(1).unwrap());
                    }
                    let loaded = LoadedOlsInput {
                        candidate: vec![true; rows],
                        outcome: LoadedNumericColumn { values: (0..rows).map(|i| {
                            let x = ((i * 7) % 109) as f64;
                            0.8 * x + (i % 31) as f64 + ((i * 3) % 7) as f64
                        }).collect(), missing: vec![false; rows] },
                        numeric_sources: vec![
                            (SourceId::new(1).unwrap(), LoadedNumericColumn {
                                values: (0..rows).map(|i| ((i * 7) % 109) as f64).collect(), missing: vec![false; rows],
                            }),
                            // Non-integer, widely spaced labels force dictionary
                            // factorization rather than the dense integer path.
                            (SourceId::new(2).unwrap(), LoadedNumericColumn {
                                values: (0..rows).map(|i| (i % 31) as f64 * 100000.5).collect(), missing: vec![false; rows],
                            }),
                        ],
                        analytic_weight: None, frequency_weight: None, probability_weight: None,
                    };
                    let clusters = vec![OlsClusterTermV1 {
                        label: "g".to_owned(), sources: vec![SourceId::new(2).unwrap()],
                    }];
                    let construction = crate::ols_preparation_memory::OlsPreparationMemory::estimate(rows, 3, false, 1, clusters.len()).unwrap()
                        .with_construction(rows, &action.request, &clusters).unwrap();
                    let bound = construction.preparation_peak_bytes(0).unwrap();
                    construction.admit_numeric_arrays(Some(bound)).unwrap();
                    assert_eq!(construction.admit_numeric_arrays(Some(bound-1)).unwrap_err().code, ErrorCode::Allocation);
                    let request = OlsClusteredRequestV1 { ols: action.request, clusters };
                    team.install(|| {
                        let start = instrumentation::begin_observation();
                        // Use the complex topology even for the intercept-only
                        // control, exercising construction overlap and validation.
                        let prepared = prepare_categorical_ols_clustered_data_owned(&request, loaded).unwrap();
                        let peak = crate::ffi::allocation_usage().peak_bytes - start.allocation_current_bytes;
                        assert!(u128::from(peak) <= bound, "construction {peak} > {bound}");
                        for mode in [PreparedOlsExecutionMode::Fast, PreparedOlsExecutionMode::Low] {
                            let mut fit = action.fit;
                            fit.execution.mode = mode;
                            fit.absorb.retain_coefficients = false;
                            fit.absorb.solver_memory_budget = None;
                            let model = crate::ols_frame_memory::OlsFrameMemory::estimate(rows,
                                prepared.ols().model().columns.len(), prepared.ols().model().coefficients.len(),
                                if vce == OlsVceKind::Clustered { prepared.clusters().len() } else { 0 },
                                fit.execution.poolsize, vce != OlsVceKind::Iid).unwrap();
                            let start = instrumentation::begin_observation();
                            let (solver_peak, owned) = match vce {
                                OlsVceKind::Iid => {
                                    let r = fit_prepared_categorical_ols_unadjusted_map(prepared.ols(),
                                        PreparedOlsUnadjustedConfig { fit, ..Default::default() }).unwrap();
                                    (r.fit().solver_workspace_peak().bytes(), r.owned_buffer_bytes())
                                },
                                OlsVceKind::Robust => {
                                    let r = fit_prepared_categorical_ols_robust_map(prepared.ols(),
                                        PreparedOlsRobustConfig { fit, ..Default::default() }).unwrap();
                                    (r.fit().solver_workspace_peak().bytes(), r.owned_buffer_bytes())
                                },
                                OlsVceKind::Clustered => {
                                    let r = fit_prepared_categorical_ols_clustered_map(&prepared,
                                        PreparedOlsClusteredConfig { fit, ..Default::default() }).unwrap();
                                    (r.fit().solver_workspace_peak().bytes(), r.owned_buffer_bytes())
                                },
                            };
                            let peak = crate::ffi::allocation_usage().peak_bytes - start.allocation_current_bytes;
                            let frame = if mode == PreparedOlsExecutionMode::Fast { model.fast_bytes } else { model.low_bytes };
                            let total = frame + u128::from(solver_peak.expect("modeled fixture solver"));
                            assert!(u128::from(peak) <= total, "frame {peak} > {total}; owner {owned}");
                        }
                    }).unwrap();
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "ffi")]
    fn action_metadata_matches_independent_deallocation_and_counts_spare_capacity() {
        let _guard = instrumentation::lock_action_observation();
        for (args, mut spi) in [
            fixture(OlsVceKind::Clustered),
            factor_scatter_fixture(),
            saved_output_fixture(),
        ] {
            let mut action = EstimateActionSpec::parse(&args)
                .unwrap()
                .decode_with_memory_limit(&mut spi, || Ok(None))
                .unwrap();
            action.request.model.sources.reserve(13);
            action.request.model.sources[0].canonical_name.reserve(29);
            action.request.model.terms.reserve(11);
            for term in &mut action.request.model.terms {
                term.canonical_token.reserve(7);
                term.components.reserve(5);
                for component in &mut term.components {
                    component.raw_operator.reserve(3);
                    component.raw_time_series_operator.reserve(17);
                }
            }
            action.request.bindings.reserve(19);
            action.request.absorbed_terms.reserve(3);
            for term in &mut action.request.absorbed_terms {
                term.categorical_sources.reserve(5);
                term.slope_sources.reserve(7);
                term.label.reserve(19);
                term.saved_name.reserve(23);
            }
            action.clusters.reserve(7);
            for cluster in &mut action.clusters {
                cluster.label.reserve(31);
                cluster.sources.reserve(3);
            }
            action.outputs.saved_channels.reserve(17);
            let before = crate::ffi::allocation_usage();
            let bytes = action.metadata_capacity_bytes();
            assert_eq!(crate::ffi::allocation_usage().events, before.events);
            assert!(bytes + action.transport_parent_bytes > action.metadata_owner_bytes);
            drop(action);
            let after = crate::ffi::allocation_usage();
            assert_eq!(
                u128::from(before.current_bytes - after.current_bytes),
                bytes
            );
            assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
        }
    }

    #[test]
    #[cfg(feature = "ffi")]
    fn action_ledger_covers_every_ols_stage_without_reporting_allocations() {
        let _guard = instrumentation::lock_action_observation();
        let (args, mut spi) = saved_output_fixture();
        let mut action = EstimateActionSpec::parse(&args)
            .unwrap()
            .decode_with_memory_limit(&mut spi, || Ok(None))
            .unwrap();
        let base = action.memory_plan.estimate;
        let loaded = load_ols_input(&action.request, false, false, &mut spi).unwrap();
        let prepared = prepare_categorical_ols_data_owned(&action.request, loaded).unwrap();
        action
            .admit_prepared_parent(prepared.owned_buffer_bytes())
            .unwrap();
        let inference = fit_prepared_categorical_ols_unadjusted_map(
            &prepared,
            PreparedOlsUnadjustedConfig {
                fit: action.fit,
                ..Default::default()
            },
        )
        .unwrap();
        action
            .admit_output_parent(inference.owned_buffer_bytes())
            .unwrap();
        let solver = inference.fit().solver_workspace_peak();
        let before = crate::ffi::allocation_usage();
        let ledger = action.memory_ledger(solver.bytes()).unwrap();
        assert_eq!(crate::ffi::allocation_usage().events, before.events);
        let summary = ledger.summary();
        assert_eq!(summary.missing_stages, 0);
        assert!(
            summary.accounted_peak
                >= action.execution_parent_bytes + u128::from(solver.bytes().unwrap())
        );
        assert!(
            summary.accounted_peak
                >= action.output_parent_bytes + action.publication_memory.peak_bytes()
        );
        assert_eq!(summary.complete_peak, None);
        assert_eq!(summary.ratio(u64::MAX), None);
        assert_eq!(action.memory_plan.estimate, base);
        assert_eq!(spi.store_calls, 0);
    }

    #[test]
    fn output_parent_admission_adds_live_owners_and_rejects_before_stores() {
        let (args, mut spi) = saved_output_fixture();
        let mut action = EstimateActionSpec::parse(&args).unwrap()
            .decode_with_memory_limit(&mut spi, || Ok(None)).unwrap();
        action.prepared_owner_bytes = 1000;
        let parent = 3000 + action.metadata_owner_bytes;
        let peak = parent + action.publication_memory.peak_bytes();
        action.fit.absorb.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 1500, usable_bytes: peak-1,
        });
        assert_eq!(action.admit_output_parent(2000).unwrap_err().code, ErrorCode::Allocation);
        assert_eq!(action.fit.absorb.solver_memory_budget.unwrap().execution_bytes, 1500);
        action.fit.absorb.solver_memory_budget.as_mut().unwrap().usable_bytes = peak;
        action.admit_output_parent(2000).unwrap();
        assert_eq!(action.fit.absorb.solver_memory_budget.unwrap().execution_bytes, parent);
        action.admit_output_parent(2000).unwrap();
        assert_eq!(action.fit.absorb.solver_memory_budget.unwrap().execution_bytes, parent);
        assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
        assert_eq!(spi.receipt_complete(), 0.0);
    }

    #[test]
    fn prepared_parent_exact_budget_reaches_solver_and_preserves_public_base() {
        let (args, mut spi) = saved_output_fixture();
        let mut spec = EstimateActionSpec::parse(&args).unwrap();
        for mode in [MemoryMode::Fast, MemoryMode::Low] {
            spec.memory = mode;
            let unrestricted = spec.decode_with_memory_limit(&mut spi, || Ok(None)).unwrap();
            let public_base = unrestricted.memory_plan.estimate;
            let owned_bytes = public_base.fast_bytes.max(public_base.low_bytes) + 4096;
            let reconciled = unrestricted.preparation_memory.with_prepared_owner(public_base, owned_bytes).unwrap();
            let adjusted = unrestricted.frame_memory.with_parent(reconciled, owned_bytes + unrestricted.preparation_memory.frame_metadata_bytes).unwrap();
            let execution = (if mode == MemoryMode::Fast { adjusted.fast_bytes } else { adjusted.low_bytes }) + unrestricted.metadata_owner_bytes;
            let usable = (execution + unrestricted.publication_memory.peak_bytes()).max(
                unrestricted.preparation_memory.preparation_peak_bytes(unrestricted.metadata_owner_bytes).unwrap());
            let available = 512 * 1024 * 1024 + usable;
            let mut exact = spec.decode_with_memory_limit(&mut spi, || Ok(Some(available))).unwrap();
            exact.admit_prepared_parent(owned_bytes).unwrap();
            assert_eq!(exact.memory_plan.estimate, public_base);
            assert_eq!(exact.fit.absorb.solver_memory_budget.unwrap(), FeSolverMemoryBudget { execution_bytes: execution, usable_bytes: usable });
            let mut short = spec.decode_with_memory_limit(&mut spi, || Ok(Some(available-1))).unwrap();
            assert_eq!(short.admit_prepared_parent(owned_bytes).unwrap_err().code, ErrorCode::Allocation);
            assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
            assert_eq!(spi.receipt_complete(), 0.0);
        }
    }

    #[test]
    fn publication_admission_uses_reported_axis_before_reading_rows() {
        for (args, mut spi) in [
            fixture(OlsVceKind::Iid),
            factor_scatter_fixture(),
            saved_output_fixture(),
        ] {
            let mut spec = EstimateActionSpec::parse(&args).unwrap();
            for mode in [MemoryMode::Fast, MemoryMode::Low] {
                spec.memory = mode;
                let unrestricted = spec
                    .decode_with_memory_limit(&mut spi, || Ok(None))
                    .unwrap();
                let frame = unrestricted.frame_memory.with_parent(unrestricted.memory_plan.estimate, unrestricted.preparation_memory.frame_metadata_bytes).unwrap();
                let base = (if mode == MemoryMode::Fast { frame.fast_bytes } else { frame.low_bytes }) + unrestricted.metadata_owner_bytes;
                let reserve = unrestricted.publication_memory.peak_bytes();
                let usable = (base + reserve)
                    .max(unrestricted.preparation_memory.preparation_peak_bytes(unrestricted.metadata_owner_bytes).unwrap())
                    .max(unrestricted.metadata_peak_bytes);
                let limit = 512 * 1024 * 1024 + usable;
                let exact = spec
                    .decode_with_memory_limit(&mut spi, || Ok(Some(limit)))
                    .unwrap();
                assert_eq!(
                    exact.memory_plan.estimate,
                    unrestricted.memory_plan.estimate
                );
                let solver = exact.fit.absorb.solver_memory_budget.unwrap();
                assert_eq!(solver.execution_bytes, base);
                assert_eq!(solver.usable_bytes, usable);
                let error = spec
                    .decode_with_memory_limit(&mut spi, || Ok(Some(limit - 1)))
                    .unwrap_err();
                assert_eq!(error.code, ErrorCode::Allocation);
                assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
                assert_eq!(spi.receipt_complete(), 0.0);
            }
        }
    }

    #[test]
    fn saved_publication_checks_observed_capacities_before_any_store() {
        let _guard = instrumentation::lock_action_observation();
        for mode in [MemoryMode::Fast, MemoryMode::Low] {
            let (args, mut spi) = saved_output_fixture();
            let mut spec = EstimateActionSpec::parse(&args).unwrap();
            spec.memory = mode;
            let mut action = spec
                .decode_with_memory_limit(&mut spi, || Ok(None))
                .unwrap();
            let observation = ActionObservation {
                started: Instant::now(),
                resources_start: instrumentation::begin_observation(),
                preflight_seconds: 0.0,
                input_load_seconds: 0.0,
            };
            let loaded = load_ols_input(
                &action.request,
                action.candidate_all,
                action.input_complete,
                &mut spi,
            )
            .unwrap();
            let prepared = prepare_categorical_ols_data_owned(&action.request, loaded).unwrap();
            let inference = fit_prepared_categorical_ols_unadjusted_map(
                &prepared,
                PreparedOlsUnadjustedConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsUnadjustedConfig::default()
                },
            )
            .unwrap();
            let result_data = PreparedOlsResultData::categorical(&prepared);
            let derived = build_derived_observation_outputs(
                result_data,
                Some(&prepared),
                inference.fit(),
                &action.outputs,
                normalization_known_connected(inference.absorbed_dof()),
                action.fit,
            )
            .unwrap();
            let mut shadow = build_shadow_result(
                result_data,
                None,
                CertifiedOlsInference::CategoricalIid(&inference),
                action.outputs.residual.is_some(),
            )
            .unwrap();
            let runtime = build_runtime_shadow(&action, &shadow, &observation).unwrap();
            let bytes = shadow.capacity_bytes()
                + derived.capacity_bytes()
                + runtime.capacity() as u128 * std::mem::size_of::<f64>() as u128;
            assert_eq!(
                shadow.capacity_bytes(),
                action.publication_memory.shadow_bytes
            );
            assert_eq!(bytes, action.publication_memory.publication_bytes);
            drop(runtime);

            action.publication_memory.publication_bytes = bytes - 1;
            let error = precommit_and_store(&action, &shadow, &derived, &observation, &mut spi)
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
            assert_eq!((spi.store_calls, spi.receipt_complete()), (0, 0.0));
            action.publication_memory.publication_bytes = bytes;
            // Lengths and scientific payload are unchanged; excess reserved
            // capacity alone must also fail before publication starts.
            shadow.b.reserve_exact(1);
            assert!(shadow.capacity_bytes() > action.publication_memory.shadow_bytes);
            assert_eq!(
                precommit_and_store(&action, &shadow, &derived, &observation, &mut spi)
                    .unwrap_err()
                    .code,
                ErrorCode::Allocation
            );
            assert_eq!((spi.store_calls, spi.receipt_complete()), (0, 0.0));
            shadow.b.shrink_to_fit();
            precommit_and_store(&action, &shadow, &derived, &observation, &mut spi).unwrap();
            assert_eq!(spi.receipt_complete(), 1.0);
        }
    }

    #[test]
    fn guarded_iid_robust_and_clustered_actions_commit_complete_last() {
        for kind in [OlsVceKind::Iid, OlsVceKind::Robust, OlsVceKind::Clustered] {
            let (args, mut spi) = fixture(kind);
            estimate_ols_test(&args, &mut spi).expect("guarded estimate succeeds");
            assert_eq!(spi.receipt_complete(), 1.0);
            assert_eq!(spi.matrices["RREC"][0][0], RESULT_MAGIC);
            assert_eq!(spi.matrices["RREC"][0][3], kind.code());
            assert_eq!(spi.matrices["RREC"][0][6], 6.0);
            assert_eq!(spi.matrices["RREC"][0][8], 6.0);
            assert_eq!(
                spi.numeric_reads,
                if kind == OlsVceKind::Clustered {
                    24
                } else {
                    18
                },
            );
            assert!(spi.matrices["RB"][0].iter().all(|value| value.is_finite()));
            assert!(spi.matrices["RV"]
                .iter()
                .flatten()
                .all(|value| value.is_finite()));
            assert_eq!(
                spi.stores.last(),
                Some(&StoreEvent::Matrix(14, 1, RECEIPT_COMPLETE_COLUMN, 1.0,)),
            );
        }
    }

    #[test]
    fn candidate_numeric_runs_preserve_coordinates_and_bound_bulk_reads() {
        let (_, mut spi) = fixture(OlsVceKind::Iid);
        spi.data.clear();
        let rows = INPUT_BULK_READ_CHUNK_ROWS + 300;
        let mut candidate = vec![false; rows];
        candidate[1..64].fill(true);
        candidate[65..129].fill(true);
        candidate[130..131 + INPUT_BULK_READ_CHUNK_ROWS].fill(true);
        for (row, retained) in candidate.iter().copied().enumerate() {
            if retained {
                spi.data
                    .insert((2, row + 1), (row as f64 - 100.0, row % 17 == 0));
                spi.data
                    .insert((3, row + 1), (-(row as f64), row % 23 == 0));
            }
        }
        let mut values = vec![12345.0; rows];
        let mut peer = vec![12345.0; rows];
        let mut missing = vec![true; rows];
        let mut peer_missing = vec![true; rows];
        read_numeric_columns_in_chunks(
            &mut spi,
            &[2, 3],
            &mut [&mut values, &mut peer],
            &mut [&mut missing, &mut peer_missing],
            Some(&candidate),
        )
        .expect("candidate run input");
        assert_eq!(
            spi.numeric_batches,
            vec![
                (66, 64, vec![2, 3]),
                (131, INPUT_BULK_READ_CHUNK_ROWS, vec![2, 3]),
            ]
        );
        assert_eq!(
            spi.numeric_reads,
            2 * candidate.iter().filter(|row| **row).count()
        );
        for row in 0..rows {
            if candidate[row] {
                assert_eq!(
                    (values[row], missing[row]),
                    (row as f64 - 100.0, row % 17 == 0)
                );
                assert_eq!(
                    (peer[row], peer_missing[row]),
                    (-(row as f64), row % 23 == 0)
                );
            } else {
                assert_eq!(
                    (values[row], peer[row], missing[row], peer_missing[row]),
                    (12345.0, 12345.0, true, true)
                );
            }
        }
        assert!(spi.interrupt_checks > 0);
        assert_eq!(spi.store_calls, 0);
    }

    #[test]
    fn candidate_numeric_loading_rejects_bad_shapes_and_polls_empty_runs() {
        let (_, mut spi) = fixture(OlsVceKind::Iid);
        let mut values = [0.0; 4];
        let mut missing = [true; 3];
        assert!(read_numeric_columns_in_chunks(
            &mut spi,
            &[2],
            &mut [&mut values],
            &mut [&mut missing],
            None
        )
        .is_err());
        let mut missing = [true; 4];
        assert!(read_numeric_columns_in_chunks(
            &mut spi,
            &[2],
            &mut [&mut values],
            &mut [&mut missing],
            Some(&[true; 3])
        )
        .is_err());
        assert_eq!(spi.numeric_reads, 0);
        let mut values = vec![0.0; 2 * INPUT_INTERRUPT_POLL_STRIDE];
        let mut missing = vec![true; values.len()];
        let candidate = vec![false; values.len()];
        spi.interrupt_at = Some(2);
        let error = read_numeric_columns_in_chunks(
            &mut spi,
            &[2],
            &mut [&mut values],
            &mut [&mut missing],
            Some(&candidate),
        )
        .expect_err("excluded scan remains interruptible");
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(spi.numeric_reads, 0);
        assert_eq!(spi.store_calls, 0);
    }

    #[test]
    fn large_restricted_mixed_input_batches_only_numeric_candidate_runs() {
        for fragmented in [false, true] {
            let (args, mut spi) = fixture(OlsVceKind::Clustered);
            spi.nobs = INPUT_CANDIDATE_BATCH_MIN_ROWS + 32;
            spi.data.clear();
            spi.string_variables.insert(4);
            let labels = [b"z".to_vec(), b"a".to_vec(), Vec::new()];
            for row in 0..spi.nobs {
                let retained = if fragmented {
                    row % 3 == 0
                } else {
                    (65..spi.nobs - 1).contains(&row)
                };
                spi.data.insert((1, row + 1), (f64::from(retained), false));
                if retained {
                    spi.data.insert((2, row + 1), (row as f64, row % 17 == 0));
                    spi.data
                        .insert((3, row + 1), (-(row as f64), row % 23 == 0));
                    spi.string_data
                        .insert((4, row + 1), labels[row % 3].clone());
                }
            }
            let action = EstimateActionSpec::parse(&args)
                .expect("specification")
                .decode_and_validate(&mut spi)
                .expect("action");
            let loaded =
                load_ols_input(&action.request, false, false, &mut spi).expect("mixed input");
            assert_eq!(spi.numeric_batches.is_empty(), fragmented);
            assert!(spi
                .numeric_batches
                .iter()
                .all(|(_, _, positions)| positions == &[2, 3]));
            for row in 0..spi.nobs {
                if loaded.candidate[row] {
                    assert_eq!(
                        (loaded.outcome.values[row], loaded.outcome.missing[row]),
                        (row as f64, row % 17 == 0)
                    );
                    let x = &loaded.numeric_sources[0].1;
                    assert_eq!(
                        (x.values[row], x.missing[row]),
                        (-(row as f64), row % 23 == 0)
                    );
                    let cluster = &loaded.numeric_sources[1].1;
                    assert_eq!(cluster.missing[row], row % 3 == 2);
                    if !cluster.missing[row] {
                        let code = if !fragmented && row % 3 == 0 {
                            1.0
                        } else {
                            0.0
                        };
                        assert_eq!(cluster.values[row], code);
                    }
                } else {
                    assert_eq!(
                        (loaded.outcome.values[row], loaded.outcome.missing[row]),
                        (0.0, false)
                    );
                    let x = &loaded.numeric_sources[0].1;
                    assert_eq!((x.values[row], x.missing[row]), (0.0, false));
                    // The existing finalizer also rebases excluded string
                    // placeholders. They retain their original code-zero
                    // meaning, while the candidate marker remains authority.
                    let cluster = &loaded.numeric_sources[1].1;
                    assert_eq!(
                        (cluster.values[row], cluster.missing[row]),
                        (if fragmented { 0.0 } else { 1.0 }, false)
                    );
                }
            }
            assert_eq!(spi.store_calls, 0);
        }
    }

    #[test]
    fn complete_candidate_universe_skips_candidate_spi_reads() {
        let (mut args, mut spi) = fixture(OlsVceKind::Iid);
        replace_argument(&mut args, "candidate_all", "candidate_all=1".to_owned());
        replace_argument(&mut args, "input_complete", "input_complete=1".to_owned());

        estimate_ols_test(&args, &mut spi).expect("complete-candidate estimate succeeds");

        // Six observations times outcome plus one source. The candidate
        // coordinate remains present on the wire but is never read.
        assert_eq!(spi.numeric_reads, 12);
        assert_eq!(spi.matrices["RREC"][0][6], 6.0);
        assert_eq!(spi.matrices["RREC"][0][7], 6.0);
        assert_eq!(spi.matrices["RREC"][0][8], 6.0);
        assert_eq!(spi.receipt_complete(), 1.0);
    }

    #[test]
    fn complete_input_certificate_disagreement_fails_before_output() {
        let (mut args, mut spi) = fixture(OlsVceKind::Iid);
        replace_argument(&mut args, "candidate_all", "candidate_all=1".to_owned());
        replace_argument(&mut args, "input_complete", "input_complete=1".to_owned());
        spi.data.insert((2, 3), (0.0, true));

        let error = estimate_ols_test(&args, &mut spi)
            .expect_err("false complete-input certificate rejected");

        assert_eq!(error.code, ErrorCode::InvalidData);
        assert_eq!(spi.store_calls, 0);
    }

    #[test]
    fn low_memory_execution_matches_fast_across_inference_modes() {
        for kind in [OlsVceKind::Iid, OlsVceKind::Robust, OlsVceKind::Clustered] {
            let (fast_args, mut fast_spi) = fixture(kind);
            estimate_ols_test(&fast_args, &mut fast_spi).expect("fast estimate succeeds");

            let (mut low_args, mut low_spi) = fixture(kind);
            replace_argument(&mut low_args, "memory", "memory=low".to_owned());
            replace_argument(&mut low_args, "poolsize", "poolsize=1".to_owned());
            estimate_ols_test(&low_args, &mut low_spi).expect("low estimate succeeds");

            for matrix in ["RB", "RV", "RSTATUS", "RFIT", "RDOF", "RFE", "RCLUSTER"] {
                let fast = &fast_spi.matrices[matrix];
                let low = &low_spi.matrices[matrix];
                assert_eq!(fast.len(), low.len(), "{matrix} row count");
                for (fast_row, low_row) in fast.iter().zip(low) {
                    assert_eq!(fast_row.len(), low_row.len(), "{matrix} column count");
                    for (fast_value, low_value) in fast_row.iter().zip(low_row) {
                        let scale = 1.0_f64.max(fast_value.abs()).max(low_value.abs());
                        assert!(
                            (fast_value - low_value).abs() <= 5e-11 * scale,
                            "{matrix}: fast={fast_value}, low={low_value}"
                        );
                    }
                }
            }
            assert_eq!(fast_spi.receipt_complete(), 1.0);
            assert_eq!(low_spi.receipt_complete(), 1.0);
            let fast_observations = fast_spi
                .stores
                .iter()
                .take_while(|event| matches!(event, StoreEvent::Numeric(..)));
            let low_observations = low_spi
                .stores
                .iter()
                .take_while(|event| matches!(event, StoreEvent::Numeric(..)));
            for (fast_event, low_event) in fast_observations.zip(low_observations) {
                match (fast_event, low_event) {
                    (
                        StoreEvent::Numeric(fast_variable, fast_observation, fast_value),
                        StoreEvent::Numeric(low_variable, low_observation, low_value),
                    ) => {
                        assert_eq!(
                            (fast_variable, fast_observation),
                            (low_variable, low_observation)
                        );
                        let scale = 1.0_f64.max(fast_value.abs()).max(low_value.abs());
                        assert!((fast_value - low_value).abs() <= 5e-11 * scale);
                    }
                    _ => unreachable!("observation prefix contains only numeric stores"),
                }
            }
        }
    }

    #[test]
    fn deterministic_thread_counts_preserve_scientific_results_and_are_reported() {
        for kind in [OlsVceKind::Iid, OlsVceKind::Robust, OlsVceKind::Clustered] {
            let (single_args, mut single_spi) = fixture(kind);
            estimate_ols_test(&single_args, &mut single_spi).expect("single-thread estimate");

            for threads in [2, 4, 8, 16] {
                let (mut parallel_args, mut parallel_spi) = fixture(kind);
                replace_argument(&mut parallel_args, "threads", format!("threads={threads}"));
                estimate_ols_test(&parallel_args, &mut parallel_spi).expect("parallel estimate");

                for matrix in ["RB", "RV", "RSTATUS", "RFIT", "RDOF", "RFE", "RCLUSTER"] {
                    assert_eq!(
                        single_spi.matrices[matrix], parallel_spi.matrices[matrix],
                        "{matrix} differs at {threads} threads"
                    );
                }
                assert_eq!(
                    parallel_spi.matrices["RRUNTIME"][27],
                    vec![1.0, threads as f64]
                );
                assert_eq!(
                    parallel_spi.matrices["RRUNTIME"][28],
                    vec![1.0, threads as f64]
                );
                assert_eq!(parallel_spi.matrices["RRUNTIME"][29], vec![1.0, 1.0]);
                assert_eq!(parallel_spi.receipt_complete(), 1.0);
            }
        }
    }

    #[test]
    fn saved_outputs_allow_a_generated_column_omitted_by_absorption() {
        for memory in ["fast", "low"] {
            let (mut args, mut spi) = saved_output_fixture();
            replace_argument(&mut args, "memory", format!("memory={memory}"));
            for observation in 1..=spi.nobs {
                let group = spi.data[&(4, observation)].0;
                spi.data.insert((3, observation), (group, false));
            }
            estimate_ols_test(&args, &mut spi).expect("omitted generated column has no xb term");
            assert_eq!(spi.matrices["RSTATUS"][0][1], 3.0);
            assert_eq!(spi.matrices["RB"][0][0], 0.0);
            let overall = (1..=spi.nobs)
                .map(|row| spi.data[&(2, row)].0)
                .sum::<f64>()
                / spi.nobs as f64;
            for observation in 1..=spi.nobs {
                let first = (observation - 1) / 2 * 2 + 1;
                let mean = (spi.data[&(2, first)].0 + spi.data[&(2, first + 1)].0) / 2.0;
                let y = spi.data[&(2, observation)].0;
                assert!((stored_numeric(&spi, 6, observation) - (y - mean)).abs() <= 1e-11);
                assert!((stored_numeric(&spi, 8, observation) - overall).abs() <= 1e-11);
                assert!((stored_numeric(&spi, 9, observation) - mean).abs() <= 1e-11);
                assert!((stored_numeric(&spi, 10, observation) - (mean - overall)).abs() <= 1e-11);
                assert!((stored_numeric(&spi, 11, observation) - (mean - overall)).abs() <= 1e-11);
            }
            assert_eq!(spi.receipt_complete(), 1.0);
        }
    }

    #[test]
    fn saved_fe_and_prediction_outputs_are_native_identity_consistent() {
        let (args, mut spi) = saved_output_fixture();
        estimate_ols_test(&args, &mut spi).expect("saved-output estimate succeeds");

        let mut saved_sum = 0.0;
        for observation in 1..=spi.nobs {
            let y = spi.data[&(2, observation)].0;
            let sample = stored_numeric(&spi, 5, observation);
            let residual = stored_numeric(&spi, 6, observation);
            let reason = stored_numeric(&spi, 7, observation);
            let xb = stored_numeric(&spi, 8, observation);
            let xbd = stored_numeric(&spi, 9, observation);
            let absorbed = stored_numeric(&spi, 10, observation);
            let saved = stored_numeric(&spi, 11, observation);
            assert_eq!(sample, 1.0);
            assert_eq!(reason, 0.0);
            assert!((saved - absorbed).abs() <= 1e-11);
            assert!((xbd - xb - absorbed).abs() <= 1e-11);
            assert!((y - xbd - residual).abs() <= 1e-11);
            saved_sum += saved;
        }
        assert!(saved_sum.abs() <= 1e-11);
        assert_eq!(stored_numeric(&spi, 11, 1), stored_numeric(&spi, 11, 2));
        assert_eq!(stored_numeric(&spi, 11, 3), stored_numeric(&spi, 11, 4));
        assert_eq!(stored_numeric(&spi, 11, 5), stored_numeric(&spi, 11, 6));
        assert_eq!(spi.receipt_complete(), 1.0);
        assert_eq!(
            spi.stores.last(),
            Some(&StoreEvent::Matrix(14, 1, RECEIPT_COMPLETE_COLUMN, 1.0)),
        );
    }

    #[test]
    fn saved_only_fast_path_matches_fully_materialized_outputs() {
        let (full_args, mut full_spi) = saved_output_fixture();
        estimate_ols_test(&full_args, &mut full_spi).expect("full saved-output estimate succeeds");

        let (mut lean_args, mut lean_spi) = saved_output_fixture();
        replace_argument(&mut lean_args, "output_xb", "output_xb=0".to_owned());
        replace_argument(&mut lean_args, "output_xbd", "output_xbd=0".to_owned());
        replace_argument(
            &mut lean_args,
            "output_absorbed",
            "output_absorbed=0".to_owned(),
        );
        replace_argument(
            &mut lean_args,
            "output_saved_channels",
            "output_saved_channels=1:8".to_owned(),
        );
        lean_spi.nvars = 8;
        estimate_ols_test(&lean_args, &mut lean_spi).expect("saved-only estimate succeeds");

        for observation in 1..=full_spi.nobs {
            assert_eq!(
                stored_numeric(&lean_spi, 8, observation),
                stored_numeric(&full_spi, 11, observation)
            );
        }
        assert_eq!(lean_spi.receipt_complete(), 1.0);
    }

    #[test]
    fn saved_output_store_failures_leave_receipt_incomplete() {
        let (args, mut successful) = saved_output_fixture();
        estimate_ols_test(&args, &mut successful).expect("saved-output baseline succeeds");
        let observation_stores = successful
            .stores
            .iter()
            .take_while(|event| matches!(event, StoreEvent::Numeric(..)))
            .count();
        assert_eq!(observation_stores, 36);
        for failing_ordinal in 1..=observation_stores {
            let (args, mut spi) = saved_output_fixture();
            spi.fail_store_at = Some(failing_ordinal);
            let error = estimate_ols_test(&args, &mut spi).expect_err("store failure propagates");
            assert_eq!(error.code, ErrorCode::Internal, "failure {failing_ordinal}");
            assert_eq!(spi.receipt_complete(), 0.0, "failure {failing_ordinal}");
            assert_eq!(spi.store_calls, failing_ordinal);
        }
    }

    #[test]
    fn fixed_width_string_cluster_is_encoded_natively_and_strl_fails_pre_read() {
        let (args, mut spi) = fixture(OlsVceKind::Clustered);
        spi.string_variables.insert(4);
        for (observation, value) in ["b", "a", "b", "c", "a", "c"].into_iter().enumerate() {
            spi.data.remove(&(4, observation + 1));
            spi.string_data
                .insert((4, observation + 1), value.as_bytes().to_vec());
        }
        estimate_ols_test(&args, &mut spi).expect("fixed-width string cluster succeeds");
        assert_eq!(spi.numeric_reads, 18);
        assert_eq!(spi.matrices["RREC"][0][6], 6.0);
        assert_eq!(spi.matrices["RCLUSTER"][0][2], 3.0);
        assert_eq!(spi.receipt_complete(), 1.0);

        let (args, mut strl) = fixture(OlsVceKind::Clustered);
        strl.string_variables.insert(4);
        strl.strl_variables.insert(4);
        let error = estimate_ols_test(&args, &mut strl).expect_err("strL must fail closed");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert_eq!(strl.numeric_reads, 0);
        assert_eq!(strl.store_calls, 0);
    }

    #[test]
    fn fixed_width_string_codes_are_byte_exact_sorted_and_missing_aware() {
        let (_, mut spi) = fixture(OlsVceKind::Iid);
        spi.string_data = BTreeMap::from([
            ((3, 1), b"a#b".to_vec()),
            ((3, 2), b"a".to_vec()),
            ((3, 3), b"".to_vec()),
            ((3, 4), b"b".to_vec()),
            ((3, 5), b"a#b".to_vec()),
        ]);
        let mut loader = OlsSourceLoader::FixedWidthString {
            column: empty_loaded_column(5, "test string codes").expect("column"),
            provisional_codes: StringDictionary::new(),
        };
        let mut memory = InputMemory::new(None, 0).unwrap();
        for observation in 1..=5 {
            loader
                .read_into(&mut spi, 3, observation, observation - 1, &mut memory)
                .expect("string read");
        }
        let encoded = loader.finalize(&mut memory).expect("deterministic recode");
        assert_eq!(encoded.values, vec![1.0, 0.0, 0.0, 2.0, 1.0]);
        assert_eq!(encoded.missing, vec![false, false, true, false, false]);
    }

    #[test]
    fn string_loading_budget_and_interruption_leave_no_public_stores() {
        fn run(usable: Option<u128>, interrupted: bool) -> Result<(LoadedOlsInput, u128)> {
            let (args, mut spi) = fixture(OlsVceKind::Clustered);
            spi.string_variables.insert(4);
            for row in 1..=6 {
                spi.data.remove(&(4, row));
                spi.string_data.insert((4, row), format!("group-{}", row % 3).into_bytes());
            }
            let spec = EstimateActionSpec::parse(&args)?;
            let action = spec.decode_and_validate(&mut spi)?;
            spi.interrupt_checks = 0;
            spi.interrupt_at = interrupted.then_some(1);
            let mut memory = loading_memory(&action.request, spi.nobs, 0, action.metadata_capacity_bytes(), usable)?;
            let result = load_ols_input_with_memory(&action.request, false, false, &mut memory, &mut spi);
            assert_eq!(spi.store_calls, 0);
            Ok((result?, memory.peak()))
        }
        let (original, peak) = run(None, false).unwrap();
        let (bounded, same_peak) = run(Some(peak), false).unwrap();
        assert_eq!(original, bounded);
        assert_eq!(peak, same_peak);
        assert_eq!(run(Some(peak - 1), false).unwrap_err().code, ErrorCode::Allocation);
        assert_eq!(run(Some(peak), true).unwrap_err().code, ErrorCode::Interrupted);
    }

    #[test]
    #[cfg(feature = "ffi")]
    fn decoder_admission_precedes_allocations_and_bounds_live_compiler_frames() {
        let _guard = crate::instrumentation::lock_action_observation();
        for fixture in [fixture(OlsVceKind::Iid), fixture(OlsVceKind::Clustered), saved_output_fixture()] {
            let (args, mut spi) = fixture;
            let spec = EstimateActionSpec::parse(&args).unwrap();
            let bound = spec.metadata_shape().peak_bytes().unwrap();
            let (start, _) = crate::ffi::begin_ols_allocation_observation();
            let action = spec.decode_with_memory_limit(&mut spi, || Ok(None)).unwrap();
            let observed = crate::ffi::allocation_usage().peak_bytes.saturating_sub(start.current_bytes);
            assert!(u128::from(observed) <= bound, "decoder {observed} exceeds {bound}");
            drop(action);
            // These fixtures are below the 20% headroom crossover, so this
            // supplies exactly one byte less than the decoder's admission.
            assert!(bound < 512 * 1024 * 1024);
            let (start, _) = crate::ffi::begin_ols_allocation_observation();
            let error = spec.decode_with_memory_limit(&mut spi, || Ok(Some(512 * 1024 * 1024 + bound - 1))).unwrap_err();
            assert_eq!(error.code, ErrorCode::Allocation);
            assert_eq!(spi.numeric_reads, 0);
            assert_eq!(spi.store_calls, 0);
            let observed = crate::ffi::allocation_usage().peak_bytes.saturating_sub(start.current_bytes);
            assert!(observed < 1024, "budget failure allocated decoder storage");
        }
    }

    #[cfg(feature = "ffi")]
    #[test]
    fn malformed_cluster_arity_cannot_reserve_outside_wire_admission() {
        let _guard = crate::instrumentation::lock_action_observation();
        let (args, mut spi) = fixture(OlsVceKind::Clustered);
        let spec = EstimateActionSpec::parse(&args).unwrap();
        spi.matrices.get_mut(&spec.cluster_term_matrix).unwrap()[0][1] = 1e12;
        let (start, _) = crate::ffi::begin_ols_allocation_observation();
        let error = spec.decode_with_memory_limit(&mut spi, || Ok(None)).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        let peak = crate::ffi::allocation_usage().peak_bytes - start.current_bytes;
        assert!(u128::from(peak) < spec.metadata_shape().peak_bytes().unwrap());
        assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
    }

    #[test]
    fn native_coefficient_shadow_certification_rejects_every_scatter_family() {
        validate_shadow_coefficients(&coefficient_shadow()).expect("valid native shadow");

        let mut duplicate = coefficient_shadow();
        duplicate.coefficient_status[7] = 1.0;
        assert!(validate_shadow_coefficients(&duplicate).is_err());

        let mut asymmetric = coefficient_shadow();
        asymmetric.covariance[1] += 1.0;
        assert!(validate_shadow_coefficients(&asymmetric).is_err());

        let mut negative_diagonal = coefficient_shadow();
        negative_diagonal.covariance[0] = -1.0;
        assert!(validate_shadow_coefficients(&negative_diagonal).is_err());

        let mut marker = coefficient_shadow();
        marker.omission_v_marker[1] = 1.0;
        assert!(validate_shadow_coefficients(&marker).is_err());

        let mut omitted = coefficient_shadow();
        omitted.coefficient_status[1] = 2.0;
        omitted.coefficient_status[3] = 0.0;
        omitted.b[0] = 0.0;
        omitted.omission_b_marker[0] = 0.0;
        omitted.omission_v_marker[0] = 0.0;
        assert!(validate_shadow_coefficients(&omitted).is_err());
    }

    #[test]
    fn native_defined_value_table_certification_rejects_malformed_rows() {
        validate_defined_value_table(&[1.0, 2.0, 0.0, 0.0], "test").expect("valid table");
        for malformed in [
            vec![2.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, f64::NAN],
            vec![1.0],
        ] {
            assert!(validate_defined_value_table(&malformed, "test").is_err());
        }
    }

    #[test]
    fn licensed_stata_bundle_matches_ordinary_argv_and_rejects_malformed_pre_read() {
        let (args, mut ordinary) = fixture(OlsVceKind::Iid);
        estimate_ols_test(&args, &mut ordinary).expect("ordinary argv succeeds");

        let (args, mut packed) = fixture(OlsVceKind::Iid);
        estimate_ols_test(&bundled(&args), &mut packed).expect("bundle succeeds");
        let (args, mut local) = fixture(OlsVceKind::Iid);
        let bundle = bundled(&args)
            .pop()
            .unwrap()
            .strip_prefix("bundle=")
            .unwrap()
            .as_bytes()
            .to_vec();
        let local_args = [format!("bundle_local={}", bundle.len())];
        local.local_bundle = Some(bundle);
        estimate_ols_test(&local_args, &mut local).expect("local bundle succeeds");
        assert_eq!(local.macro_reads, 1);
        assert_eq!(ordinary.macro_reads, 0);
        assert_eq!(packed.macro_reads, 0);
        for name in [
            "RB", "RV", "RSTATUS", "RMARKB", "RMARKV", "RFIT", "RDOF", "RFE", "RCLUSTER", "RSUBSET",
            "RCERT", "RREC",
        ] {
            assert_eq!(packed.matrices[name], ordinary.matrices[name], "{name}");
            assert_eq!(
                local.matrices[name], ordinary.matrices[name],
                "local {name}"
            );
        }
        for runtime in [
            &ordinary.matrices["RRUNTIME"],
            &packed.matrices["RRUNTIME"],
            &local.matrices["RRUNTIME"],
        ] {
            assert_eq!(runtime[0], [1.0, 1.0]);
            assert_eq!(runtime[1], [1.0, 1.0]);
            assert_eq!(runtime[2], [1.0, 10.0]);
            assert_eq!(runtime[21], [1.0, 1e-8]);
            assert_eq!(runtime[22], [1.0, 1e-8]);
            assert_eq!(runtime[23], [1.0, 16_000.0]);
            assert!(runtime
                .iter()
                .all(|row| matches!(row[0], 0.0 | 1.0) && row[1].is_finite()));
        }
        for row in 0..SOLVER_ROWS {
            for column in 0..SOLVER_COLUMNS {
                if !matches!(column, 8 | 9) {
                    assert_eq!(
                        packed.matrices["RSOLVER"][row][column],
                        ordinary.matrices["RSOLVER"][row][column]
                    );
                }
            }
        }
        assert_eq!(packed.store_calls, ordinary.store_calls);
        assert_eq!(packed.stores.last(), ordinary.stores.last());
        assert_eq!(packed.numeric_reads, ordinary.numeric_reads);
        assert_eq!(local.numeric_reads, ordinary.numeric_reads);
        assert_eq!(local.stores.last(), ordinary.stores.last());
        assert_eq!(local.store_calls, ordinary.store_calls);

        let malformed = [
            "bundle=0:",
            "bundle=01:3:a=b",
            "bundle=1:03:a=b",
            "bundle=1:4:a=b",
            "bundle=1:3:a=bx",
            "bundle=1:0:",
            "bundle=1:x:a=b",
            "bundle=1:1:\u{00e9}",
            "bundle=2:3:a=b3:a=b",
            "bundle=1:3:abc",
        ];
        for argument in malformed {
            let (_, mut spi) = fixture(OlsVceKind::Iid);
            let error = estimate_ols_test(&[argument.to_owned()], &mut spi)
                .expect_err("malformed or incomplete bundle is rejected");
            assert!(matches!(
                error.code,
                ErrorCode::Syntax | ErrorCode::InvalidArgument
            ));
            assert_eq!(spi.numeric_reads, 0, "{argument}");
            assert_eq!(spi.store_calls, 0, "{argument}");
            assert_eq!(spi.receipt_complete(), 0.0, "{argument}");
        }

        let (args, mut unknown_spi) = fixture(OlsVceKind::Iid);
        let mut unknown = args;
        unknown.push("surprise=1".to_owned());
        let error = estimate_ols_test(&bundled(&unknown), &mut unknown_spi)
            .expect_err("unknown bundled field is rejected");
        assert_eq!(error.code, ErrorCode::Syntax);
        assert_eq!(unknown_spi.numeric_reads, 0);
        assert_eq!(unknown_spi.store_calls, 0);

        let (args, mut mixed_spi) = fixture(OlsVceKind::Iid);
        let mut mixed = bundled(&args);
        mixed.push("ado=0.5.0".to_owned());
        let error =
            estimate_ols_test(&mixed, &mut mixed_spi).expect_err("mixed bundle and argv are rejected");
        assert_eq!(error.code, ErrorCode::Syntax);
        assert_eq!(mixed_spi.numeric_reads, 0);
        assert_eq!(mixed_spi.store_calls, 0);
    }

    #[test]
    fn local_estimation_bundle_validates_capacity_bytes_and_framing_before_reads() {
        for descriptor in [
            "",
            "0",
            "01",
            "+1",
            "-1",
            "1.0",
            "1 ",
            "16777217",
            "999999999999999999999999",
        ] {
            let (_, mut spi) = fixture(OlsVceKind::Iid);
            assert!(estimate_ols_test(&[format!("bundle_local={descriptor}")], &mut spi).is_err());
            assert_eq!(
                (spi.macro_reads, spi.numeric_reads, spi.store_calls),
                (0, 0, 0)
            );
        }
        let (_, mut spi) = fixture(OlsVceKind::Iid);
        assert!(estimate_ols_test(
            &["bundle_local=6".to_owned(), "ado=0.5.0".to_owned()],
            &mut spi
        )
        .is_err());
        assert_eq!(spi.macro_reads, 0);
        for (payload, declared) in [
            (None, 6),
            (Some(b"1:3:a=b".to_vec()), 6),
            (Some(b"1:3:a=b".to_vec()), 8),
            (Some(b"1:3:a\0b".to_vec()), 7),
            (Some(vec![0xff; 7]), 7),
            (Some(b"1:9:a=b".to_vec()), 7),
            (Some(b"1:3:a=b".to_vec()), 7),
        ] {
            let (_, mut spi) = fixture(OlsVceKind::Iid);
            spi.local_bundle = payload;
            assert!(estimate_ols_test(&[format!("bundle_local={declared}")], &mut spi).is_err());
            assert_eq!(spi.macro_reads, 1);
            assert_eq!((spi.numeric_reads, spi.store_calls), (0, 0));
            assert_eq!(spi.receipt_complete(), 0.0);
        }
        let (args, mut overlong) = fixture(OlsVceKind::Iid);
        let mut payload = format!("{}:", args.len());
        for value in &args {
            payload.push_str(&format!("{}:{value}", value.len()));
        }
        let declared = payload.len();
        payload.push_str("trailing-data");
        overlong.local_bundle = Some(payload.into_bytes());
        assert!(estimate_ols_test(&[format!("bundle_local={declared}")], &mut overlong).is_err());
        assert_eq!((overlong.macro_reads, overlong.numeric_reads, overlong.store_calls), (1, 0, 0));

        let (_, mut spi) = fixture(OlsVceKind::Iid);
        let payload = "1:4:a=é";
        spi.local_bundle = Some(payload.as_bytes().to_vec());
        assert_eq!(
            read_local_estimate_bundle(&[format!("bundle_local={}", payload.len())], &mut spi).unwrap(),
            Some(format!("bundle={payload}"))
        );
    }

    #[test]
    fn every_output_store_failure_leaves_receipt_incomplete() {
        let (args, mut successful) = fixture(OlsVceKind::Iid);
        estimate_ols_test(&args, &mut successful).expect("baseline estimate succeeds");
        let total_stores = successful.store_calls;
        assert_eq!(total_stores, 284 + crate::memory_report::CELLS);
        assert_eq!(successful.stores.capacity(), 512);

        for failing_ordinal in 1..=total_stores {
            let (args, mut spi) = fixture(OlsVceKind::Iid);
            let prepared_capacity = spi.stores.capacity();
            spi.fail_store_at = Some(failing_ordinal);
            let error = estimate_ols_test(&args, &mut spi).expect_err("store failure propagates");
            assert_eq!(error.code, ErrorCode::Internal, "failure {failing_ordinal}");
            assert_eq!(spi.receipt_complete(), 0.0, "failure {failing_ordinal}");
            assert_eq!(spi.store_calls, failing_ordinal);
            assert_eq!(spi.stores.capacity(), prepared_capacity);
        }
    }

    #[test]
    fn nonpositive_residual_dof_stores_nothing_and_leaves_receipt_incomplete() {
        let (args, mut spi) = fixture(OlsVceKind::Iid);
        spi.nobs = 2;
        let error = estimate_ols_test(&args, &mut spi)
            .expect_err("a saturated model must fail before result storage");
        assert_eq!(error.code, ErrorCode::InsufficientObservations);
        assert_eq!(spi.store_calls, 0);
        assert!(spi.stores.is_empty());
        assert_eq!(spi.receipt_complete(), 0.0);
    }

    #[test]
    fn omitted_factor_slot_is_zero_scattered_and_noconstant_layout_is_exact() {
        let (factor_args, mut factor_spi) = factor_scatter_fixture();
        estimate_ols_test(&factor_args, &mut factor_spi).expect("factor estimate succeeds");
        assert_eq!(factor_spi.matrices["RREC"][0][4], 3.0);
        assert_eq!(factor_spi.matrices["RB"][0][0], 0.0);
        assert_eq!(factor_spi.matrices["RSTATUS"][0][0], 1.0);
        assert_eq!(factor_spi.matrices["RSTATUS"][0][1], 2.0);
        assert_eq!(factor_spi.matrices["RSTATUS"][0][3], 0.0);
        assert_eq!(factor_spi.matrices["RMARKB"][0][0], 0.0);
        assert_eq!(factor_spi.matrices["RMARKV"][0][0], 0.0);
        for index in 0..3 {
            assert_eq!(factor_spi.matrices["RV"][0][index], 0.0);
            assert_eq!(factor_spi.matrices["RV"][index][0], 0.0);
        }
        assert!(matches!(factor_spi.matrices["RSTATUS"][1][1], 1.0 | 5.0));
        assert!(matches!(factor_spi.matrices["RSTATUS"][2][1], 1.0 | 5.0));
        for index in 1..3 {
            assert_eq!(factor_spi.matrices["RMARKB"][0][index], 1.0);
            assert_eq!(factor_spi.matrices["RMARKV"][index][index], 1.0);
        }

        let (mut noconstant_args, mut noconstant_spi) = fixture(OlsVceKind::Iid);
        replace_argument(
            &mut noconstant_args,
            "include_intercept",
            "include_intercept=0".to_owned(),
        );
        noconstant_spi
            .matrices
            .insert("RB".to_owned(), zero_matrix(1, 1));
        noconstant_spi
            .matrices
            .insert("RV".to_owned(), zero_matrix(1, 1));
        noconstant_spi.matrices.insert(
            "RSTATUS".to_owned(),
            zero_matrix(1, COEFFICIENT_STATUS_COLUMNS),
        );
        noconstant_spi
            .matrices
            .insert("RMARKB".to_owned(), zero_matrix(1, 1));
        noconstant_spi
            .matrices
            .insert("RMARKV".to_owned(), zero_matrix(1, 1));
        estimate_ols_test(&noconstant_args, &mut noconstant_spi)
            .expect("noconstant estimate succeeds");
        assert_eq!(noconstant_spi.matrices["RREC"][0][4], 1.0);
        assert_eq!(noconstant_spi.matrices["RREC"][0][5], 1.0);
        assert_eq!(noconstant_spi.matrices["RSTATUS"][0][1], 1.0);
        assert_eq!(noconstant_spi.receipt_complete(), 1.0);
    }

    #[test]
    fn handshake_layout_and_interrupt_fail_before_any_output_write() {
        let (mut handshake_args, mut handshake_spi) = fixture(OlsVceKind::Iid);
        handshake_args
            .iter_mut()
            .find(|argument| argument.starts_with("plugin="))
            .expect("plugin argument")
            .clone_from(&"plugin=wrong".to_owned());
        let error = estimate_ols_test(&handshake_args, &mut handshake_spi)
            .expect_err("plugin mismatch rejected");
        assert_eq!(error.code, ErrorCode::ProtocolMismatch);
        assert_eq!(handshake_spi.numeric_reads, 0);
        assert_eq!(handshake_spi.store_calls, 0);

        let (mut old_action_args, mut old_action_spi) = fixture(OlsVceKind::Iid);
        replace_argument(
            &mut old_action_args,
            "protocol_minor",
            "protocol_minor=0".to_owned(),
        );
        let error = estimate_ols_test(&old_action_args, &mut old_action_spi)
            .expect_err("pre-estimation action protocol rejected");
        assert_eq!(error.code, ErrorCode::ProtocolMismatch);
        assert_eq!(old_action_spi.numeric_reads, 0);
        assert_eq!(old_action_spi.store_calls, 0);

        let (mut old_result_args, mut old_result_spi) = fixture(OlsVceKind::Iid);
        replace_argument(
            &mut old_result_args,
            "result_minor",
            "result_minor=0".to_owned(),
        );
        let error = estimate_ols_test(&old_result_args, &mut old_result_spi)
            .expect_err("pre-rebase result wire rejected");
        assert_eq!(error.code, ErrorCode::ProtocolMismatch);
        assert_eq!(old_result_spi.numeric_reads, 0);
        assert_eq!(old_result_spi.store_calls, 0);

        let (layout_args, mut layout_spi) = fixture(OlsVceKind::Iid);
        layout_spi
            .matrices
            .insert("RV".to_owned(), zero_matrix(1, 2));
        let error = estimate_ols_test(&layout_args, &mut layout_spi)
            .expect_err("bad result layout rejected");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(layout_spi.numeric_reads, 0);
        assert_eq!(layout_spi.store_calls, 0);

        let (stale_args, mut stale_spi) = fixture(OlsVceKind::Iid);
        stale_spi.matrices.get_mut("RREC").expect("receipt")[0][RECEIPT_COMPLETE_COLUMN - 1] = 1.0;
        let error = estimate_ols_test(&stale_args, &mut stale_spi)
            .expect_err("stale completed receipt rejected");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(stale_spi.numeric_reads, 0);
        assert_eq!(stale_spi.store_calls, 0);

        let (sentinel_args, mut sentinel_spi) = fixture(OlsVceKind::Iid);
        sentinel_spi.matrices.get_mut("CT").expect("cluster terms")[0][0] = 1.0;
        let error = estimate_ols_test(&sentinel_args, &mut sentinel_spi)
            .expect_err("nonzero empty-cluster sentinel rejected");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(sentinel_spi.numeric_reads, 0);
        assert_eq!(sentinel_spi.store_calls, 0);

        let (interrupt_args, mut interrupt_spi) = fixture(OlsVceKind::Iid);
        let (baseline_args, mut baseline_spi) = fixture(OlsVceKind::Iid);
        estimate_ols_test(&baseline_args, &mut baseline_spi).expect("baseline estimate succeeds");
        interrupt_spi.interrupt_at = Some(baseline_spi.interrupt_checks);
        let error = estimate_ols_test(&interrupt_args, &mut interrupt_spi)
            .expect_err("last precommit interrupt propagates");
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(interrupt_spi.store_calls, 0);
        assert_eq!(interrupt_spi.receipt_complete(), 0.0);
    }

    #[test]
    fn shadow_fe_identity_accepts_intercept_interaction_and_slope_rows() {
        for row in [
            [1.0, 1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 0.0],
            [2.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0],
            [3.0, 1.0, 0.0, 6.0, 1.0, 5.0, 1.0, 0.0],
            [4.0, 1.0, 3.0, 8.0, 0.0, 8.0, 1.0, 0.0],
        ] {
            validate_shadow_fixed_effect_row(&row).expect("valid FE shadow row");
        }
    }

    #[test]
    fn shadow_fe_identity_rejects_malformed_combinations() {
        for row in [
            [1.0, 1.0, 0.0, 3.0, 1.0, 2.0, 1.0, 0.0],
            [2.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0],
            [3.0, 0.0, 0.0, 6.0, 1.0, 5.0, 1.0, 0.0],
            [3.0, 1.0, 2.0, 6.0, 1.0, 5.0, 1.0, 0.0],
            [3.0, 1.0, 0.0, 2.0, 3.0, 0.0, 1.0, 0.0],
            [3.0, 1.0, 0.0, 2.0, 1.0, 2.0, 1.0, 0.0],
            [4.0, 0.0, 3.0, 8.0, 0.0, 8.0, 1.0, 0.0],
            [4.0, 1.0, 0.0, 8.0, 0.0, 8.0, 1.0, 0.0],
            [4.0, 1.0, 3.0, 8.0, 1.0, 8.0, 1.0, 0.0],
        ] {
            assert!(validate_shadow_fixed_effect_row(&row).is_err());
        }
    }
}
