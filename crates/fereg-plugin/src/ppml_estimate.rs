use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::time::Instant;

use crate::estimate::{
    imposed_memory_limit_bytes, load_ols_input, parse_bool, parse_nonnegative_u32,
    parse_positive_f64, parse_positive_u32, parse_positive_usize, parse_u16, parse_usize,
    read_bindings, read_local_estimate_bundle, read_numeric_columns_in_chunks, read_table, required,
};
use crate::input::{PreparedMatrixStoreName, SpiApi};
use crate::instrumentation::{
    ppml_action_allocation_observation, ppml_allocator_snapshot_probe,
    ppml_linear_phase_allocator_probe, ppml_memory_preflight_probe, ppml_phase_allocator_probe,
};
use crate::ppml_result::{
    build_certified_ppml_shadow, finalize_ppml_runtime_shadow, CertifiedPpmlShadowContext,
    PpmlRuntimeFinalObservation, PpmlShadowResult, CERTIFICATE_ROWS, CLUSTER_COLUMNS,
    COEFFICIENT_STATUS_COLUMNS, DOF_ROWS, DROP_COUNT_COLUMNS, DROP_COUNT_ROWS, FE_COLUMNS,
    FIT_ROWS, INFERENCE_ROWS, RUNTIME_ROWS, SEPARATION_COLUMNS, SEPARATION_SEQUENCE_COLUMNS,
    SEPARATION_SEQUENCE_ROWS, SOLVER_COLUMNS, SOLVER_ROWS, SUBSET_COLUMNS,
};
use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::metadata::{
    AbsorbedTerm, CompiledTermGraph, ExpandedTerm, MetadataFlag, ModelWireV2, SourceEntry,
    SourceId, TermComponent, ABSORB_COMPONENT_ROW_WIDTH, ABSORB_TERM_ROW_WIDTH,
    COMPONENT_ROW_WIDTH, SOURCE_ROW_WIDTH, TERM_ROW_WIDTH,
};
use fereg_core::ols::{
    LoadedNumericColumn, OlsClusterTermV1, OlsClusteredRequestV1, OlsInputRole, OlsRequestV1,
    PreparedOlsExecutionConfig, PreparedOlsExecutionMode, PreparedOlsFitConfig,
};
#[cfg(test)]
use fereg_core::ppml::fit_prepared_ppml;
use fereg_core::ppml::{
    fit_prepared_ppml_interruptible, LoadedPpmlInput, PpmlConfig, PpmlFixedEffectChannelKind,
    PpmlInterruptPoint, PpmlMemoryConfig, PpmlPhase, PpmlPhaseAllocatorEvent,
    PpmlPhaseAllocatorProbe, PpmlResidualKind, PpmlSolverFamily, PpmlVceRequest,
    PreparedPpmlAdjustmentKind, PreparedPpmlOutputRequest, PreparedPpmlRequestV1,
};
use fereg_core::ppml_separation::{SeparationConfig, SeparationMethods};
use fereg_core::protocol::{
    Handshake, ACTION_PROTOCOL_VERSION, PLUGIN_VERSION, PPML_REQUEST_WIRE_VERSION,
    PPML_RESULT_RECEIPT_COLUMNS, PPML_RESULT_RECEIPT_COMPLETE_COLUMN,
    PPML_RESULT_RECEIPT_COMPLETE_VALUE, PPML_RESULT_WIRE_VERSION,
};
use fereg_core::{MemoryMode, ThreadConfig};

pub const ACTION_ESTIMATE_PPML: &str = "estimate-ppml";
const CONFIRMATION: &str = "feppml-estimation";
const CLUSTER_TERM_ROW_WIDTH: usize = 2;
const CLUSTER_COMPONENT_ROW_WIDTH: usize = 3;
const MAX_PPML_PROTOCOL_FIELDS: usize = 83;
const NATIVE_BREAK_MARKER: &str = "FEPPML_NATIVE_BREAK_ARMED\n";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum PpmlDeveloperInjection {
    #[default]
    None,
    Panic,
    BreakNative,
    Allocation,
}

#[cfg(test)]
static TEST_LAST_EMBEDDING_RESIDENT_BYTES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

struct PublicationPhaseScope {
    probe: Option<PpmlPhaseAllocatorProbe>,
    open: bool,
}

impl PublicationPhaseScope {
    fn begin(probe: Option<PpmlPhaseAllocatorProbe>) -> Self {
        if let Some(probe) = probe {
            let _ = probe(PpmlPhase::Publication, PpmlPhaseAllocatorEvent::Begin);
        }
        Self {
            probe,
            open: probe.is_some(),
        }
    }

    fn finish(&mut self) -> Option<u64> {
        let probe = self.probe?;
        self.open = false;
        Some(probe(PpmlPhase::Publication, PpmlPhaseAllocatorEvent::End))
    }
}

impl Drop for PublicationPhaseScope {
    fn drop(&mut self) {
        if self.open {
            if let Some(probe) = self.probe {
                let _ = probe(PpmlPhase::Publication, PpmlPhaseAllocatorEvent::End);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ResultMatrices {
    b: String,
    covariance: String,
    coefficient_status: String,
    fit: String,
    dof: String,
    fixed_effects: String,
    clusters: String,
    subsets: String,
    separation: String,
    inference: String,
    drop_counts: String,
    separation_sequence: String,
    certificates: String,
    solver: String,
    runtime: String,
    receipt: String,
}

#[derive(Debug)]
struct PreparedResultMatrices {
    b: PreparedMatrixStoreName,
    covariance: PreparedMatrixStoreName,
    coefficient_status: PreparedMatrixStoreName,
    fit: PreparedMatrixStoreName,
    dof: PreparedMatrixStoreName,
    fixed_effects: PreparedMatrixStoreName,
    clusters: PreparedMatrixStoreName,
    subsets: PreparedMatrixStoreName,
    separation: PreparedMatrixStoreName,
    inference: PreparedMatrixStoreName,
    drop_counts: PreparedMatrixStoreName,
    separation_sequence: PreparedMatrixStoreName,
    certificates: PreparedMatrixStoreName,
    solver: PreparedMatrixStoreName,
    runtime: PreparedMatrixStoreName,
    receipt: PreparedMatrixStoreName,
}

#[derive(Debug, Clone, Copy)]
struct ResultMatrixDimensions {
    coefficient_count: usize,
    fixed_effect_rows: usize,
    cluster_dimensions: usize,
    cluster_subset_rows: usize,
    separation_rows: usize,
}

#[derive(Debug, Clone, Default)]
struct ObservationOutputs {
    sample: u32,
    residual: Option<u32>,
    xb: Option<u32>,
    absorbed: Option<u32>,
    eta: Option<u32>,
    mean: Option<u32>,
    drop_reason: Option<u32>,
    saved_channels: Vec<Option<u32>>,
}

impl ObservationOutputs {
    fn positions(&self) -> impl Iterator<Item = u32> + '_ {
        std::iter::once(Some(self.sample))
            .chain([
                self.residual,
                self.xb,
                self.absorbed,
                self.eta,
                self.mean,
                self.drop_reason,
            ])
            .chain(self.saved_channels.iter().copied())
            .flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdjustmentBinding {
    None,
    Offset(u32),
    Exposure(u32),
}

impl AdjustmentBinding {
    const fn position(self) -> Option<u32> {
        match self {
            Self::None => None,
            Self::Offset(position) | Self::Exposure(position) => Some(position),
        }
    }

    const fn kind(self) -> PreparedPpmlAdjustmentKind {
        match self {
            Self::None => PreparedPpmlAdjustmentKind::None,
            Self::Offset(_) => PreparedPpmlAdjustmentKind::Offset,
            Self::Exposure(_) => PreparedPpmlAdjustmentKind::Exposure,
        }
    }
}

#[derive(Debug)]
struct EstimatePpmlAction {
    request: OlsRequestV1,
    clusters: Vec<OlsClusterTermV1>,
    vce: PpmlVceRequest,
    candidate_all: bool,
    input_complete: bool,
    adjustment: AdjustmentBinding,
    residual: Option<PpmlResidualKind>,
    config: PpmlConfig,
    linear: PreparedOlsFitConfig,
    memory: PpmlMemoryConfig,
    solver: PpmlSolverFamily,
    separation: SeparationConfig,
    requested_observation_columns: usize,
    publication_matrix_cells: usize,
    allocation_audit: bool,
    outputs: ObservationOutputs,
    matrices: PreparedResultMatrices,
    injection: PpmlDeveloperInjection,
    break_marker_token: String,
}

#[derive(Debug)]
struct EstimatePpmlSpec {
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
    adjustment: String,
    adjustment_position: u32,
    residual: String,
    vce: String,
    memory: MemoryMode,
    poolsize: usize,
    threads: usize,
    deterministic: bool,
    allocation_audit: bool,
    solver: String,
    tolerance: f64,
    iterate: usize,
    separation: String,
    separation_tolerance: f64,
    separation_max_iterations: usize,
    simplex_max_iterations: usize,
    simplex_max_tableau_cells: usize,
    outputs: ObservationOutputs,
    matrices: ResultMatrices,
    injection: PpmlDeveloperInjection,
    break_marker_token: String,
}

pub fn estimate_ppml_test(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {
    #[cfg(test)]
    let _action_observation_guard = crate::instrumentation::lock_action_observation();
    let started = Instant::now();
    let resources_start = crate::instrumentation::begin_ppml_observation();
    let (action_probe, action_start, action_previous_baseline) =
        ppml_action_allocation_observation(resources_start);
    let local_bundle = read_local_estimate_bundle(args, spi)?.map(|value| [value]);
    let specification = EstimatePpmlSpec::parse(
        local_bundle.as_ref().map_or(args, |values| values.as_slice()),
    )?;
    drop(local_bundle);
    let action = specification.decode_and_validate(spi)?;
    if action.injection == PpmlDeveloperInjection::Panic {
        panic!("injected hidden PPML transaction panic");
    }
    let loaded = load_ols_input(
        &action.request,
        action.candidate_all,
        action.input_complete,
        spi,
    )?;
    let adjustment = load_adjustment(action.adjustment, &loaded.candidate, spi)?;
    spi.check_interrupted()?;
    let phase_probe = ppml_phase_allocator_probe(action.allocation_audit);
    let mut prepared_request = PreparedPpmlRequestV1 {
        ols: fallible_clone_ols_request(&action.request)?,
        clusters: fallible_clone_clusters(&action.clusters)?,
        vce: action.vce,
        ppml: action.config,
        linear: action.linear,
        memory: action.memory,
        memory_preflight_probe: ppml_memory_preflight_probe()?,
        allocation_snapshot_probe: ppml_allocator_snapshot_probe(action.allocation_audit),
        phase_allocation_probe: phase_probe,
        linear_phase_allocation_probe: ppml_linear_phase_allocator_probe(action.allocation_audit),
        // Action lifetime includes the embedding shadow and staged SPI
        // stores, so the plugin owns this later observation boundary.
        action_allocation_snapshot_probe: None,
        action_allocation_start: None,
        action_allocation_previous_baseline_bytes: None,
        separation: action.separation,
        output: PreparedPpmlOutputRequest {
            residual: action.residual,
            xb: action.outputs.xb.is_some(),
            absorbed: action.outputs.absorbed.is_some(),
            eta: action.outputs.eta.is_some(),
            mean: action.outputs.mean.is_some(),
            requested_observation_columns: action.requested_observation_columns,
            publication_matrix_cells: action.publication_matrix_cells,
        },
        solver: action.solver,
        inject_fe_recovery_allocation_failure: action.injection
            == PpmlDeveloperInjection::Allocation,
    };
    let prepared_input = LoadedPpmlInput {
        ols: loaded,
        adjustment,
        adjustment_kind: action.adjustment.kind(),
    };
    // The shared linear kernel retains one process-local Rayon team. Build
    // or switch it and synchronously prime every worker before the embedding
    // snapshot. `ThreadPoolBuilder::build` may return while workers are still
    // initializing; an empty single-worker install is therefore not a strong
    // enough boundary. The broadcast makes the complete allocator footprint
    // observable as live embedding state instead of racing into an arbitrary
    // first-solve phase.
    fereg_core::ThreadTeam::new(action.linear.execution.threads).install(|| {
        drop(rayon::broadcast(|_| ()));
    })?;
    // Core phases are measured relative to the action-start allocator
    // baseline, while the decoded action, loaded columns, and owned request
    // remain live when Preparation begins.  Snapshot only after constructing
    // every core-owned input so preflight charges that exact resident delta
    // to every phase instead of guessing allocator overhead.
    match (action_probe, action_start) {
        (Some(probe), Some(start)) => {
            let before_fit = probe();
            let embedding_resident_bytes =
                before_fit.current_bytes.saturating_sub(start.current_bytes);
            prepared_request.memory.embedding_resident_bytes = u128::from(embedding_resident_bytes);
            #[cfg(test)]
            TEST_LAST_EMBEDDING_RESIDENT_BYTES.store(
                embedding_resident_bytes,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        (None, None) => {}
        _ => {
            return Err(internal(
                "PPML action allocator observation is partial before core entry",
            ));
        }
    }
    let mut native_break_armed = false;
    let mut interrupt = |point: PpmlInterruptPoint| {
        // This closure borrows the non-Send/non-Sync SPI session and is called
        // only by serial core boundaries.  It can therefore never migrate to
        // a Rayon worker or invoke Stata from a worker thread.
        spi.check_interrupted()?;
        if action.injection == PpmlDeveloperInjection::BreakNative
            && !native_break_armed
            && point == PpmlInterruptPoint::IrlsIteration
        {
            native_break_armed = true;
            create_native_break_marker(&action.break_marker_token)?;
            spi.display(NATIVE_BREAK_MARKER)?;
            // Synchronize the interactive fixture after actual core entry.
            // The controller sends Ctrl-C only after observing the marker;
            // remain on Stata's caller thread until its poll reports Break.
            loop {
                spi.check_interrupted()?;
                std::thread::yield_now();
            }
        }
        Ok(())
    };
    let result = fit_prepared_ppml_interruptible(prepared_request, prepared_input, &mut interrupt)?;
    let publication_extension_started = Instant::now();
    let mut publication_phase = PublicationPhaseScope::begin(phase_probe);
    let mut shadow = build_certified_ppml_shadow(
        &result,
        CertifiedPpmlShadowContext {
            residual: action.residual,
            xb_requested: action.outputs.xb.is_some(),
            separation_methods: action.separation.methods,
            drop_singletons: action.separation.drop_singletons,
            elapsed_seconds: started.elapsed().as_secs_f64(),
        },
    )?;
    precommit(&action, &shadow, spi)?;
    store_shadow_prefix(&action, &shadow, spi)?;
    let action_end = action_probe.map(|probe| probe());
    let publication_peak_bytes = publication_phase.finish();
    finalize_ppml_runtime_shadow(
        &mut shadow.runtime,
        PpmlRuntimeFinalObservation {
            total_seconds: started.elapsed().as_secs_f64(),
            publication_extension_seconds: publication_extension_started.elapsed().as_secs_f64(),
            publication_peak_bytes,
            action_start,
            action_end,
            previous_baseline_bytes: action_previous_baseline,
            modeled_selected_bytes: result.memory().plan.selected_bytes(),
        },
    )?;
    store_runtime_and_complete(&action, &shadow, spi)
}

impl EstimatePpmlSpec {
    fn parse(args: &[String]) -> Result<Self> {
        let values = parse_ppml_estimate_key_values(args)?;
        let handshake = Handshake::from_key_values(&values)?;
        handshake.validate()?;
        if handshake.protocol != ACTION_PROTOCOL_VERSION {
            return Err(protocol(format!(
                "{ACTION_ESTIMATE_PPML} requires action protocol {}.{}",
                ACTION_PROTOCOL_VERSION.major, ACTION_PROTOCOL_VERSION.minor,
            )));
        }
        if required(&values, "confirm")? != CONFIRMATION {
            return Err(unsupported(format!(
                "{ACTION_ESTIMATE_PPML} requires confirm={CONFIRMATION}",
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
        if result_major != PPML_RESULT_WIRE_VERSION.major
            || result_minor != PPML_RESULT_WIRE_VERSION.minor
        {
            return Err(protocol(format!(
                "PPML result wire {result_major}.{result_minor} is incompatible with {}.{}",
                PPML_RESULT_WIRE_VERSION.major, PPML_RESULT_WIRE_VERSION.minor,
            )));
        }
        let request_major = parse_u16(&values, "request_major")?;
        let request_minor = parse_u16(&values, "request_minor")?;
        if request_major != PPML_REQUEST_WIRE_VERSION.major
            || request_minor != PPML_REQUEST_WIRE_VERSION.minor
        {
            return Err(protocol(format!(
                "PPML request wire {request_major}.{request_minor} is incompatible with {}.{}",
                PPML_REQUEST_WIRE_VERSION.major, PPML_REQUEST_WIRE_VERSION.minor,
            )));
        }
        let source_count = parse_usize(&values, "source_count")?;
        let term_count = parse_usize(&values, "term_count")?;
        let component_count = parse_usize(&values, "component_count")?;
        let absorb_term_count = parse_usize(&values, "absorb_term_count")?;
        let absorb_component_count = parse_usize(&values, "absorb_component_count")?;
        let cluster_term_count = parse_usize(&values, "cluster_term_count")?;
        let cluster_component_count = parse_usize(&values, "cluster_component_count")?;
        let specification = Self {
            model_major: f64::from(parse_u16(&values, "model_major")?),
            model_minor: f64::from(parse_u16(&values, "model_minor")?),
            source_matrix: required(&values, "source_matrix")?.to_owned(),
            source_count,
            term_matrix: required(&values, "term_matrix")?.to_owned(),
            term_count,
            component_matrix: required(&values, "component_matrix")?.to_owned(),
            component_count,
            source_names: decode_ppml_registry(required(&values, "source_names")?, source_count)?,
            canonical_tokens: decode_ppml_registry(
                required(&values, "canonical_tokens")?,
                term_count,
            )?,
            raw_component_operators: decode_ppml_registry(
                required(&values, "raw_component_operators")?,
                component_count,
            )?,
            raw_time_series_operators: decode_ppml_registry(
                required(&values, "raw_time_series_operators")?,
                component_count,
            )?,
            absorb_term_matrix: required(&values, "absorb_term_matrix")?.to_owned(),
            absorb_term_count,
            absorb_component_matrix: required(&values, "absorb_component_matrix")?.to_owned(),
            absorb_component_count,
            absorb_labels: decode_ppml_registry(
                required(&values, "absorb_labels")?,
                absorb_term_count,
            )?,
            absorb_saved_names: decode_ppml_registry(
                required(&values, "absorb_saved_names")?,
                absorb_term_count,
            )?,
            cluster_term_matrix: required(&values, "cluster_term_matrix")?.to_owned(),
            cluster_term_count,
            cluster_component_matrix: required(&values, "cluster_component_matrix")?.to_owned(),
            cluster_component_count,
            cluster_labels: decode_ppml_registry(
                required(&values, "cluster_labels")?,
                cluster_term_count,
            )?,
            binding_matrix: required(&values, "binding_matrix")?.to_owned(),
            binding_count: parse_usize(&values, "binding_count")?,
            include_intercept: parse_bool(&values, "include_intercept")?,
            candidate_all: parse_bool(&values, "candidate_all")?,
            input_complete: parse_bool(&values, "input_complete")?,
            keep_singletons: parse_bool(&values, "keep_singletons")?,
            adjustment: required(&values, "adjustment")?.to_ascii_lowercase(),
            adjustment_position: parse_nonnegative_u32(&values, "adjustment_position")?,
            residual: required(&values, "residual")?.to_ascii_lowercase(),
            vce: required(&values, "vce")?.to_ascii_lowercase(),
            memory: MemoryMode::parse(required(&values, "memory")?)?,
            poolsize: parse_positive_usize(&values, "poolsize")?,
            threads: parse_usize(&values, "threads")?,
            deterministic: parse_bool(&values, "deterministic")?,
            allocation_audit: values
                .get("allocation_audit")
                .map(|_| parse_bool(&values, "allocation_audit"))
                .transpose()?
                .unwrap_or(false),
            solver: required(&values, "solver")?.to_ascii_lowercase(),
            tolerance: parse_positive_f64(&values, "tolerance")?,
            iterate: parse_positive_usize(&values, "iterate")?,
            separation: required(&values, "separation")?.to_ascii_lowercase(),
            separation_tolerance: parse_positive_f64(&values, "separation_tolerance")?,
            separation_max_iterations: parse_positive_usize(&values, "separation_max_iterations")?,
            simplex_max_iterations: parse_positive_usize(&values, "simplex_max_iterations")?,
            simplex_max_tableau_cells: parse_positive_usize(&values, "simplex_max_tableau_cells")?,
            outputs: ObservationOutputs {
                sample: parse_positive_u32(&values, "output_sample")?,
                residual: optional_position(&values, "output_residual")?,
                xb: optional_position(&values, "output_xb")?,
                absorbed: optional_position(&values, "output_absorbed")?,
                eta: optional_position(&values, "output_eta")?,
                mean: optional_position(&values, "output_mean")?,
                drop_reason: optional_position(&values, "output_drop_reason")?,
                saved_channels: decode_output_position_registry(
                    required(&values, "output_saved_channels")?,
                    parse_usize(&values, "output_saved_count")?,
                )?,
            },
            matrices: ResultMatrices {
                b: required(&values, "output_b_matrix")?.to_owned(),
                covariance: required(&values, "output_v_matrix")?.to_owned(),
                coefficient_status: required(&values, "output_status_matrix")?.to_owned(),
                fit: required(&values, "output_fit_matrix")?.to_owned(),
                dof: required(&values, "output_dof_matrix")?.to_owned(),
                fixed_effects: required(&values, "output_fe_matrix")?.to_owned(),
                clusters: required(&values, "output_cluster_matrix")?.to_owned(),
                subsets: required(&values, "output_subset_matrix")?.to_owned(),
                separation: required(&values, "output_separation_matrix")?.to_owned(),
                inference: required(&values, "output_inference_matrix")?.to_owned(),
                drop_counts: required(&values, "output_drop_counts_matrix")?.to_owned(),
                separation_sequence: required(&values, "output_separation_sequence_matrix")?
                    .to_owned(),
                certificates: required(&values, "output_certificate_matrix")?.to_owned(),
                solver: required(&values, "output_solver_matrix")?.to_owned(),
                runtime: required(&values, "output_runtime_matrix")?.to_owned(),
                receipt: required(&values, "output_receipt_matrix")?.to_owned(),
            },
            injection: match values.get("inject").map(String::as_str).unwrap_or("") {
                "" => PpmlDeveloperInjection::None,
                "panic" => PpmlDeveloperInjection::Panic,
                "breaknative" => PpmlDeveloperInjection::BreakNative,
                "allocation" => PpmlDeveloperInjection::Allocation,
                other => {
                    return Err(syntax(format!(
                        "unknown hidden PPML injection stage {other:?}",
                    )));
                }
            },
            break_marker_token: values
                .get("break_marker_token")
                .cloned()
                .unwrap_or_default(),
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
            "request_major",
            "request_minor",
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
            "adjustment",
            "adjustment_position",
            "residual",
            "vce",
            "memory",
            "poolsize",
            "threads",
            "deterministic",
            "allocation_audit",
            "solver",
            "tolerance",
            "iterate",
            "separation",
            "separation_tolerance",
            "separation_max_iterations",
            "simplex_max_iterations",
            "simplex_max_tableau_cells",
            "output_sample",
            "output_residual",
            "output_xb",
            "output_absorbed",
            "output_eta",
            "output_mean",
            "output_drop_reason",
            "output_saved_count",
            "output_saved_channels",
            "output_b_matrix",
            "output_v_matrix",
            "output_status_matrix",
            "output_fit_matrix",
            "output_dof_matrix",
            "output_fe_matrix",
            "output_cluster_matrix",
            "output_subset_matrix",
            "output_separation_matrix",
            "output_inference_matrix",
            "output_drop_counts_matrix",
            "output_separation_sequence_matrix",
            "output_certificate_matrix",
            "output_solver_matrix",
            "output_runtime_matrix",
            "output_receipt_matrix",
            "inject",
            "break_marker_token",
        ];
        for key in values.keys() {
            if !ALLOWED.contains(&key.as_str()) {
                return Err(syntax(format!(
                    "unknown {ACTION_ESTIMATE_PPML} field {key}",
                )));
            }
        }
        Ok(())
    }

    fn decode_and_validate(mut self, spi: &mut dyn SpiApi) -> Result<EstimatePpmlAction> {
        if !self.include_intercept {
            return Err(unsupported(
                "hidden estimate-ppml requires the structural constant",
            ));
        }
        if self.tolerance >= 1.0 || self.separation_tolerance >= 1.0 {
            return Err(syntax(
                "PPML tolerances must lie strictly between zero and one",
            ));
        }
        let expects_break_marker = self.injection == PpmlDeveloperInjection::BreakNative;
        let has_break_marker = !self.break_marker_token.is_empty();
        if expects_break_marker != has_break_marker {
            return Err(syntax(
                "breaknative requires exactly one hidden break_marker_token",
            ));
        }
        if !self.break_marker_token.is_empty()
            && (self.break_marker_token.len() != 32
                || !self
                    .break_marker_token
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        {
            return Err(syntax(
                "hidden break_marker_token must be exactly 32 lowercase hexadecimal characters",
            ));
        }

        let wire = ModelWireV2 {
            major: self.model_major,
            minor: self.model_minor,
            sources: read_table(
                spi,
                &self.source_matrix,
                self.source_count,
                SOURCE_ROW_WIDTH,
                "PPML source",
            )?,
            terms: read_table(
                spi,
                &self.term_matrix,
                self.term_count,
                TERM_ROW_WIDTH,
                "PPML term",
            )?,
            components: read_table(
                spi,
                &self.component_matrix,
                self.component_count,
                COMPONENT_ROW_WIDTH,
                "PPML component",
            )?,
            source_names: std::mem::take(&mut self.source_names),
            canonical_tokens: std::mem::take(&mut self.canonical_tokens),
            raw_component_operators: std::mem::take(&mut self.raw_component_operators),
            raw_time_series_operators: std::mem::take(&mut self.raw_time_series_operators),
            absorb_terms: read_table(
                spi,
                &self.absorb_term_matrix,
                self.absorb_term_count,
                ABSORB_TERM_ROW_WIDTH,
                "PPML absorbed term",
            )?,
            absorb_components: read_table(
                spi,
                &self.absorb_component_matrix,
                self.absorb_component_count,
                ABSORB_COMPONENT_ROW_WIDTH,
                "PPML absorbed component",
            )?,
            absorb_labels: std::mem::take(&mut self.absorb_labels),
            absorb_saved_names: std::mem::take(&mut self.absorb_saved_names),
        };
        let decoded = wire.decode_specification()?;
        decoded.model.validate_executable_metadata()?;
        let request = OlsRequestV1 {
            model: decoded.model,
            absorbed_terms: decoded.absorbed_terms,
            bindings: read_bindings(spi, &self.binding_matrix, self.binding_count)?,
            include_intercept: self.include_intercept,
            keep_singletons: self.keep_singletons,
        };
        let clusters = read_ppml_clusters(&self, spi, &request)?;
        let vce = parse_ppml_vce(&self.vce, &clusters)?;
        validate_ppml_weights(&request)?;

        let adjustment = match (self.adjustment.as_str(), self.adjustment_position) {
            ("none", 0) => AdjustmentBinding::None,
            ("offset", position) if position > 0 => AdjustmentBinding::Offset(position),
            ("exposure", position) if position > 0 => AdjustmentBinding::Exposure(position),
            ("none", _) | ("offset" | "exposure", 0) => {
                return Err(syntax(
                    "PPML adjustment kind and adjustment_position disagree",
                ));
            }
            _ => {
                return Err(syntax("PPML adjustment must be none, offset, or exposure"));
            }
        };
        validate_adjustment_binding(adjustment, spi)?;
        let residual = parse_residual_request(&self.residual, self.outputs.residual)?;
        let outputs = std::mem::take(&mut self.outputs);
        validate_saved_output_layout(&request, &outputs)?;
        validate_variable_layout(&request, adjustment, &outputs, spi)?;
        let coefficient_count = request
            .model
            .terms
            .len()
            .checked_add(1)
            .ok_or_else(|| allocation("PPML coefficient count overflows"))?;
        let separation_methods = parse_separation_methods(&self.separation)?;
        let separation_rows = separation_method_capacity(separation_methods);
        let fixed_effect_rows =
            request
                .absorbed_terms
                .iter()
                .try_fold(0_usize, |total, term| {
                    total
                        .checked_add(usize::from(term.intercept) + term.slope_sources.len())
                        .ok_or_else(|| allocation("PPML fixed-effect result rows overflow"))
                })?;
        let cluster_subset_rows = cluster_subset_rows(clusters.len())?;
        let matrices = validate_matrix_layout(
            &self.matrices,
            ResultMatrixDimensions {
                coefficient_count,
                fixed_effect_rows,
                cluster_dimensions: clusters.len(),
                cluster_subset_rows,
                separation_rows,
            },
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
        let (request, clusters) = if clusters.is_empty() {
            request.validate_ppml_and_compile()?;
            (request, clusters)
        } else {
            let clustered = OlsClusteredRequestV1 {
                ols: request,
                clusters,
            };
            clustered.validate_ppml_and_compile()?;
            (clustered.ols, clustered.clusters)
        };
        let config = PpmlConfig::with_outer_controls(self.tolerance, self.iterate)?;
        let solver = self.decode_solver()?;
        let threads = ThreadConfig::from_requested(self.threads, self.deterministic)?;
        let execution_mode = match self.memory {
            MemoryMode::Low => PreparedOlsExecutionMode::Low,
            MemoryMode::Auto | MemoryMode::Fast => PreparedOlsExecutionMode::Fast,
        };
        let (absorb, fixed_effect_collinearity_tolerance) = config.shared_linear_policy();
        let linear = PreparedOlsFitConfig {
            // Separation projections and the final IRLS session must share
            // one exact FE/rank policy.  PreparedOlsFitConfig has OLS-tuned
            // defaults which intentionally differ from PpmlConfig, so copy
            // PPML's authoritative fields rather than accepting two defaults.
            absorb,
            fixed_effect_collinearity_tolerance,
            execution: PreparedOlsExecutionConfig {
                mode: execution_mode,
                poolsize: self.poolsize,
                threads,
            },
            ..PreparedOlsFitConfig::default()
        };
        let memory = PpmlMemoryConfig {
            requested: self.memory,
            available_bytes: imposed_memory_limit_bytes()?,
            poolsize: self.poolsize,
            ..PpmlMemoryConfig::default()
        };
        let separation = SeparationConfig {
            methods: separation_methods,
            tolerance: self.separation_tolerance,
            certificate_tolerance: self.separation_tolerance.min(1e-7),
            projection_tolerance: self.separation_tolerance.min(1e-10),
            max_ir_iterations: self.separation_max_iterations,
            simplex_max_iterations: self.simplex_max_iterations,
            simplex_max_tableau_cells: self.simplex_max_tableau_cells,
            drop_singletons: !self.keep_singletons,
            strict: true,
            ..SeparationConfig::default()
        };
        let requested_observation_columns = outputs
            .saved_channels
            .iter()
            .filter(|position| position.is_some())
            .count()
            .checked_add(usize::from(outputs.residual.is_some()))
            .and_then(|count| count.checked_add(usize::from(outputs.xb.is_some())))
            .and_then(|count| count.checked_add(usize::from(outputs.absorbed.is_some())))
            .and_then(|count| count.checked_add(usize::from(outputs.eta.is_some())))
            .and_then(|count| count.checked_add(usize::from(outputs.mean.is_some())))
            .ok_or_else(|| allocation("PPML requested output-column count overflows"))?;
        let publication_matrix_cells = publication_matrix_cells(&matrices)?;
        Ok(EstimatePpmlAction {
            request,
            clusters,
            vce,
            candidate_all: self.candidate_all,
            input_complete: self.input_complete,
            adjustment,
            residual,
            config,
            linear,
            memory,
            solver,
            separation,
            requested_observation_columns,
            publication_matrix_cells,
            allocation_audit: self.allocation_audit,
            outputs,
            matrices,
            injection: self.injection,
            break_marker_token: self.break_marker_token,
        })
    }

    fn decode_solver(&self) -> Result<PpmlSolverFamily> {
        match self.solver.as_str() {
            "auto" => Ok(PpmlSolverFamily::Auto),
            "map" => Ok(PpmlSolverFamily::Map),
            "lsmr" => Ok(PpmlSolverFamily::Lsmr),
            "cmg" => Ok(PpmlSolverFamily::Cmg),
            other => Err(syntax(format!(
                "solver() must be one of auto, map, lsmr, or cmg; received {other:?}"
            ))),
        }
    }
}

fn native_break_marker_path(token: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("feppml-native-break-{token}.marker"))
}

fn create_native_break_marker(token: &str) -> Result<()> {
    let path = native_break_marker_path(token);
    let mut marker = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            internal(format!(
                "unable to create hidden PPML Break marker: {error}"
            ))
        })?;
    marker
        .write_all(NATIVE_BREAK_MARKER.as_bytes())
        .and_then(|_| marker.sync_all())
        .map_err(|error| {
            internal(format!(
                "unable to commit hidden PPML Break marker: {error}"
            ))
        })
}

/// Parse the hidden PPML control plane without infallibly duplicating the
/// potentially coefficient-sized registry values. Protocol field count is
/// fixed and bounded; each key and value payload is copied only after a
/// fallible reservation.
fn parse_ppml_estimate_key_values(args: &[String]) -> Result<BTreeMap<String, String>> {
    let Some(bundle) = (args.len() == 1)
        .then(|| args[0].strip_prefix("bundle="))
        .flatten()
    else {
        if args.iter().any(|argument| argument.starts_with("bundle=")) {
            return Err(syntax(
                "guarded PPML bundle must be the only action argument",
            ));
        }
        return parse_ppml_key_value_fields(args);
    };
    let fields = decode_ppml_estimate_bundle(bundle)?;
    parse_ppml_key_value_fields(&fields)
}

fn decode_ppml_estimate_bundle(bundle: &str) -> Result<Vec<String>> {
    let bytes = bundle.as_bytes();
    let (count, mut cursor) = parse_ppml_bundle_decimal(bytes, 0, "field count")?;
    if count == 0 || count > MAX_PPML_PROTOCOL_FIELDS {
        return Err(syntax(format!(
            "guarded PPML bundle field count must be between one and {MAX_PPML_PROTOCOL_FIELDS}",
        )));
    }
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(count)
        .map_err(|_| allocation("unable to reserve guarded PPML bundle fields"))?;
    for _ in 0..count {
        let (length, payload_start) = parse_ppml_bundle_decimal(bytes, cursor, "field length")?;
        if length == 0 {
            return Err(syntax("guarded PPML bundle field is empty"));
        }
        let payload_end = payload_start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| syntax("guarded PPML bundle field is truncated"))?;
        let payload = bundle
            .get(payload_start..payload_end)
            .ok_or_else(|| syntax("guarded PPML bundle length splits UTF-8 data"))?;
        fields.push(fallible_string(payload, "guarded PPML bundle field")?);
        cursor = payload_end;
    }
    if cursor != bytes.len() {
        return Err(syntax("guarded PPML bundle has trailing data"));
    }
    Ok(fields)
}

fn parse_ppml_bundle_decimal(
    bytes: &[u8],
    start: usize,
    label: &'static str,
) -> Result<(usize, usize)> {
    let mut cursor = start;
    while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
        cursor += 1;
    }
    if cursor == start || cursor >= bytes.len() || bytes[cursor] != b':' {
        return Err(syntax(format!(
            "guarded PPML bundle {label} lacks a decimal length delimiter"
        )));
    }
    if cursor - start > 1 && bytes[start] == b'0' {
        return Err(syntax(format!(
            "guarded PPML bundle {label} is not canonically encoded"
        )));
    }
    let value = std::str::from_utf8(&bytes[start..cursor])
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| syntax(format!("guarded PPML bundle {label} overflows")))?;
    Ok((value, cursor + 1))
}

fn parse_ppml_key_value_fields(fields: &[String]) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for argument in fields {
        let (key, value) = argument.split_once('=').ok_or_else(|| {
            syntax(format!(
                "plugin argument {argument:?} must have key=value form"
            ))
        })?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(syntax(format!(
                "plugin argument {argument:?} has an empty key or value"
            )));
        }
        let mut key = fallible_string(key, "PPML protocol field name")?;
        key.make_ascii_lowercase();
        if values.contains_key(&key) {
            return Err(syntax(format!("duplicate plugin protocol field {key}")));
        }
        let value = fallible_string(value, "PPML protocol field value")?;
        values.insert(key, value);
    }
    Ok(values)
}

fn load_adjustment(
    binding: AdjustmentBinding,
    candidate: &[bool],
    spi: &mut dyn SpiApi,
) -> Result<Option<LoadedNumericColumn>> {
    let Some(position) = binding.position() else {
        return Ok(None);
    };
    let mut values = reserved_filled(candidate.len(), 0.0, "PPML adjustment values")?;
    let mut missing = reserved_filled(candidate.len(), true, "PPML adjustment missing mask")?;
    read_numeric_columns_in_chunks(
        spi,
        &[position],
        &mut [values.as_mut_slice()],
        &mut [missing.as_mut_slice()],
        Some(candidate),
    )?;
    spi.check_interrupted()?;
    Ok(Some(LoadedNumericColumn { values, missing }))
}

fn precommit(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    validate_shadow(action, shadow, spi)?;
    spi.check_interrupted()
}

fn validate_shadow(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    let dimension = shadow.coefficient_count.max(1);
    if shadow.b.len() != dimension
        || shadow.covariance.len() != dimension.saturating_mul(dimension)
        || shadow.coefficient_status.len() != dimension.saturating_mul(COEFFICIENT_STATUS_COLUMNS)
        || shadow.fit.len() != FIT_ROWS
        || shadow.dof.len() != DOF_ROWS
        || shadow.fixed_effects.len() != shadow.fixed_effect_rows.max(1).saturating_mul(FE_COLUMNS)
        || shadow.clusters.len() != shadow.cluster_rows.max(1).saturating_mul(CLUSTER_COLUMNS)
        || shadow.subsets.len() != shadow.subset_rows.max(1).saturating_mul(SUBSET_COLUMNS)
        || shadow.separation.len()
            != shadow
                .separation_rows
                .max(1)
                .saturating_mul(SEPARATION_COLUMNS)
        || shadow.inference.len() != INFERENCE_ROWS
        || shadow.drop_counts.len() != DROP_COUNT_ROWS.saturating_mul(DROP_COUNT_COLUMNS)
        || shadow.separation_sequence.len()
            != SEPARATION_SEQUENCE_ROWS.saturating_mul(SEPARATION_SEQUENCE_COLUMNS)
        || shadow.certificates.len() != CERTIFICATE_ROWS
        || shadow.solver.len() != SOLVER_ROWS.saturating_mul(SOLVER_COLUMNS)
        || shadow.runtime.len() != RUNTIME_ROWS
        || shadow.drop_reasons.len() != shadow.receipt[6] as usize
        || shadow.retained_original_rows.len() != shadow.receipt[8] as usize
        || (action.outputs.residual.is_some()
            && shadow.residuals.len() != shadow.retained_original_rows.len())
        || (action.outputs.residual.is_none() && !shadow.residuals.is_empty())
        || (action.outputs.xb.is_some() && shadow.xb.len() != shadow.retained_original_rows.len())
        || (action.outputs.xb.is_none() && !shadow.xb.is_empty())
        || (action.outputs.absorbed.is_some()
            && shadow.absorbed.len() != shadow.retained_original_rows.len())
        || (action.outputs.absorbed.is_none() && !shadow.absorbed.is_empty())
        || (action.outputs.eta.is_some() && shadow.eta.len() != shadow.retained_original_rows.len())
        || (action.outputs.eta.is_none() && !shadow.eta.is_empty())
        || (action.outputs.mean.is_some()
            && shadow.mean.len() != shadow.retained_original_rows.len())
        || (action.outputs.mean.is_none() && !shadow.mean.is_empty())
        || shadow.saved_channels.len() != action.outputs.saved_channels.len()
        || shadow
            .saved_channels
            .iter()
            .zip(&action.outputs.saved_channels)
            .any(|(values, position)| {
                position.is_some() && values.len() != shadow.retained_original_rows.len()
                    || position.is_none() && !values.is_empty()
            })
    {
        return Err(internal(
            "PPML publication shadow has inconsistent dimensions",
        ));
    }
    if action.matrices.b.rows() != 1
        || action.matrices.b.columns() != dimension
        || action.matrices.covariance.rows() != dimension
        || action.matrices.covariance.columns() != dimension
        || action.matrices.coefficient_status.rows() != dimension
        || action.matrices.coefficient_status.columns() != COEFFICIENT_STATUS_COLUMNS
        || action.matrices.fit.rows() != FIT_ROWS
        || action.matrices.fit.columns() != 1
        || action.matrices.dof.rows() != DOF_ROWS
        || action.matrices.dof.columns() != 1
        || action.matrices.fixed_effects.rows() != shadow.fixed_effect_rows.max(1)
        || action.matrices.fixed_effects.columns() != FE_COLUMNS
        || action.matrices.clusters.rows() != shadow.cluster_rows.max(1)
        || action.matrices.clusters.columns() != CLUSTER_COLUMNS
        || action.matrices.subsets.rows() != shadow.subset_rows.max(1)
        || action.matrices.subsets.columns() != SUBSET_COLUMNS
        || action.matrices.separation.rows() != shadow.separation_rows.max(1)
        || action.matrices.separation.columns() != SEPARATION_COLUMNS
        || action.matrices.inference.rows() != INFERENCE_ROWS
        || action.matrices.inference.columns() != 1
        || action.matrices.drop_counts.rows() != DROP_COUNT_ROWS
        || action.matrices.drop_counts.columns() != DROP_COUNT_COLUMNS
        || action.matrices.separation_sequence.rows() != SEPARATION_SEQUENCE_ROWS
        || action.matrices.separation_sequence.columns() != SEPARATION_SEQUENCE_COLUMNS
        || action.matrices.certificates.rows() != CERTIFICATE_ROWS
        || action.matrices.certificates.columns() != 1
        || action.matrices.solver.rows() != SOLVER_ROWS
        || action.matrices.solver.columns() != SOLVER_COLUMNS
        || action.matrices.runtime.rows() != RUNTIME_ROWS
        || action.matrices.runtime.columns() != 1
        || action.matrices.receipt.rows() != 1
        || action.matrices.receipt.columns() != PPML_RESULT_RECEIPT_COLUMNS
    {
        return Err(internal(
            "PPML publication shadow exceeds or disagrees with its preflighted stores",
        ));
    }
    validate_result_request_identity(action, shadow)?;
    validate_solver_evidence(action, shadow)?;
    validate_bounded_evidence(action, shadow)?;
    if shadow.receipt.iter().any(|value| !value.is_finite())
        || shadow.receipt[0] != fereg_core::PPML_RESULT_MAGIC
        || shadow.receipt[1] != f64::from(PPML_RESULT_WIRE_VERSION.major)
        || shadow.receipt[2] != f64::from(PPML_RESULT_WIRE_VERSION.minor)
        || shadow.receipt[PPML_RESULT_RECEIPT_COMPLETE_COLUMN - 1] != 0.0
        || shadow.receipt[4] as usize != shadow.coefficient_count
        || shadow.receipt[5] as usize != shadow.coefficient_count
        || shadow.receipt[6] as usize != shadow.drop_reasons.len()
        || shadow.receipt[7] > shadow.receipt[6]
        || shadow.receipt[8] as usize != shadow.retained_original_rows.len()
        || shadow.receipt[9] as usize != shadow.cluster_rows
        || shadow.receipt[10] as usize != shadow.fixed_effect_rows
        || shadow.receipt[11] as usize != shadow.subset_rows
        || shadow.receipt[12] != 1.0
        || action.outputs.residual.is_some() != (shadow.receipt[13] == 1.0)
        || shadow.receipt[14] != 1.0
        || shadow.receipt[16] as usize != shadow.separation_rows
        || action
            .outputs
            .saved_channels
            .iter()
            .filter(|position| position.is_some())
            .count()
            != shadow.receipt[17] as usize
        || shadow.receipt[19] as usize != INFERENCE_ROWS
        || shadow.receipt[20] as usize != DROP_COUNT_ROWS
        || shadow.receipt[21] as usize != SEPARATION_SEQUENCE_ROWS
        || shadow.receipt[22] as usize != shadow.separation_execution_count
        || shadow.receipt[23] as usize != shadow.separation_generation_count
        || shadow_hash(shadow.receipt[24], shadow.receipt[25])? != shadow.initial_generation_hash
        || shadow_hash(shadow.receipt[26], shadow.receipt[27])? != shadow.final_generation_hash
        || shadow_hash(shadow.receipt[28], shadow.receipt[29])? != shadow.separation_trace_digest
        || shadow_hash(shadow.receipt[30], shadow.receipt[31])? != shadow.generation_hash_digest
    {
        return Err(internal(
            "PPML publication receipt disagrees with the request",
        ));
    }
    for index in [
        4_usize, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 19, 20, 21, 22, 23,
    ] {
        exact_shadow_count(shadow.receipt[index], "PPML receipt count")?;
    }
    let original = exact_shadow_count(shadow.receipt[6], "PPML original observations")?;
    let candidate = exact_shadow_count(shadow.receipt[7], "PPML candidate observations")?;
    let final_observations = exact_shadow_count(shadow.receipt[8], "PPML final observations")?;
    let separated = exact_shadow_count(shadow.receipt[15], "PPML separated observations")?;
    if candidate > original
        || final_observations > candidate
        || separated > candidate - final_observations
        || !matches!(shadow.receipt[12], 0.0 | 1.0)
        || !matches!(shadow.receipt[13], 0.0 | 1.0)
        || !matches!(shadow.receipt[14], 0.0 | 1.0)
        || shadow.receipt[12] != 1.0
        || shadow.receipt[14] != 1.0
    {
        return Err(internal("PPML publication receipt is structurally invalid"));
    }
    let mut direct_separated = 0_usize;
    for row in 0..shadow.separation_rows {
        let start = row * SEPARATION_COLUMNS;
        if shadow.separation[start] != 1.0 {
            direct_separated = direct_separated
                .checked_add(exact_shadow_count(
                    shadow.separation[start + 5],
                    "PPML direct separation drops",
                )?)
                .ok_or_else(|| internal("PPML direct separation drop count overflows"))?;
        }
    }
    if direct_separated != separated {
        return Err(internal(
            "PPML separation aggregate disagrees with the receipt drop count",
        ));
    }
    if [
        shadow.residuals,
        shadow.xb,
        shadow.absorbed,
        shadow.eta,
        shadow.mean,
    ]
    .into_iter()
    .chain(shadow.saved_channels.iter().copied())
    .any(|values| {
        values
            .iter()
            .any(|value| !crate::ppml_result::is_stata_publishable(*value))
    }) {
        return Err(internal(
            "PPML publication shadow contains a nonfinite observation output",
        ));
    }
    if shadow
        .retained_original_rows
        .windows(2)
        .any(|rows| rows[0] >= rows[1])
        || shadow
            .retained_original_rows
            .iter()
            .any(|row| *row >= shadow.drop_reasons.len())
    {
        return Err(internal("PPML retained original-row map is invalid"));
    }
    for name in prepared_matrix_names(&action.matrices) {
        let (rows, columns) = spi.matrix_dimensions(name.as_str())?;
        if rows != name.rows() || columns != name.columns() {
            return Err(invalid(format!(
                "PPML output matrix {} changed dimensions before commit",
                name.as_str(),
            )));
        }
    }
    if spi.matrix_element(
        action.matrices.receipt.as_str(),
        1,
        PPML_RESULT_RECEIPT_COMPLETE_COLUMN,
    )? != 0.0
    {
        return Err(invalid(
            "PPML result receipt must be incomplete before publication",
        ));
    }
    Ok(())
}

fn validate_solver_evidence(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
) -> Result<()> {
    let expected_requested = f64::from(action.solver.code());
    let expected_selected = if action.solver == PpmlSolverFamily::Auto {
        let selected = shadow.solver[1];
        let lsmr_eligible = (2..=3).contains(&action.request.absorbed_terms.len())
            && action.memory.requested != MemoryMode::Low
            && !action.allocation_audit
            && action.request.absorbed_terms.iter().all(|term| {
                term.intercept
                    && term.slope_sources.is_empty()
                    && !term.categorical_sources.is_empty()
            });
        if selected != f64::from(PpmlSolverFamily::Map.code())
            && !(selected == f64::from(PpmlSolverFamily::Lsmr.code()) && lsmr_eligible)
        {
            return Err(internal(
                "PPML automatic solver selection is outside its admitted envelope",
            ));
        }
        selected
    } else {
        f64::from(action.solver.code())
    };
    let selected_engine = shadow.solver[2];
    let cmg_start = 3 * SOLVER_COLUMNS;
    let mut fallback_count = 0_usize;
    let mut fallback_reason = 0.0_f64;
    for row in 0..SOLVER_ROWS {
        let start = row * SOLVER_COLUMNS;
        if shadow.solver[start] != expected_requested
            || shadow.solver[start + 1] != expected_selected
            || shadow.solver[start + 2] != selected_engine
            || shadow.solver[start + 3] != shadow.solver[3]
            || shadow.solver[start + 4] != shadow.solver[4]
            || (shadow.solver[start + 5] != f64::from(PpmlSolverFamily::Map.code())
                && shadow.solver[start + 5] != f64::from(PpmlSolverFamily::Lsmr.code()))
            || shadow.solver[start + 6] != (row + 1) as f64
        {
            return Err(internal(
                "PPML solver evidence has inconsistent row identity",
            ));
        }
        for column in [7_usize, 8, 9, 10, 12] {
            exact_shadow_count(shadow.solver[start + column], "PPML solver counter")?;
        }
        let row_fallback_count =
            exact_shadow_count(shadow.solver[start + 10], "PPML solver fallback count")?;
        let row_fallback_reason = shadow.solver[start + 11];
        if ![0.0, 1.0, 2.0, 3.0].contains(&row_fallback_reason)
            || (row_fallback_count == 0) != (row_fallback_reason == 0.0)
            || (row_fallback_reason == 1.0 && row != 3)
            || (row_fallback_reason == 2.0 && row != 0)
            || (row_fallback_reason == 3.0 && row != 0 && row != 3)
        {
            return Err(internal(
                "PPML solver fallback reason is inconsistent with its engine row",
            ));
        }
        if row_fallback_reason != 0.0 {
            if fallback_reason != 0.0 && fallback_reason != row_fallback_reason {
                return Err(internal(
                    "PPML solver fallback rows disagree on the fallback reason",
                ));
            }
            fallback_reason = row_fallback_reason;
        }
        fallback_count = fallback_count
            .checked_add(row_fallback_count)
            .ok_or_else(|| internal("PPML solver fallback total overflows"))?;
        if shadow.solver[start + 13..start + 16]
            .iter()
            .any(|value| !crate::ppml_result::is_stata_publishable(*value) || *value < 0.0)
            || shadow.solver[start + 16..start + SOLVER_COLUMNS]
                .iter()
                .any(|value| crate::ppml_result::is_stata_publishable(*value) && *value < 0.0)
        {
            return Err(internal("PPML solver evidence contains an invalid metric"));
        }
    }
    let selected_engine_valid = if expected_selected == 2.0 {
        selected_engine == 1.0
    } else if expected_selected == 3.0 {
        selected_engine == 2.0 || selected_engine == 3.0
    } else if expected_selected == 7.0 {
        selected_engine == 4.0
    } else {
        false
    };
    let checkpoint = shadow.solver[5];
    let checkpoint_valid = checkpoint == f64::from(PpmlSolverFamily::Map.code())
        || (checkpoint == f64::from(PpmlSolverFamily::Lsmr.code())
            && (expected_selected == f64::from(PpmlSolverFamily::Lsmr.code())
                || expected_selected == f64::from(PpmlSolverFamily::Cmg.code())));
    let no_fallback = fallback_reason == 0.0
        && fallback_count == 0
        && shadow.solver[3] == expected_selected
        && shadow.solver[4] == selected_engine;
    let cmg_fallback = fallback_reason == 1.0
        && fallback_count == 1
        && expected_selected == f64::from(PpmlSolverFamily::Cmg.code())
        && selected_engine == 4.0
        && shadow.solver[3] == f64::from(PpmlSolverFamily::Map.code())
        && shadow.solver[4] == 1.0
        && exact_shadow_count(shadow.solver[cmg_start + 10], "PPML CMG fallback count")?
            == fallback_count;
    let checkpoint_fallback = fallback_reason == 2.0
        && fallback_count > 0
        && checkpoint == f64::from(PpmlSolverFamily::Lsmr.code())
        && shadow.solver[3] == expected_selected
        && shadow.solver[4] == selected_engine
        && exact_shadow_count(shadow.solver[10], "PPML checkpoint fallback count")?
            == fallback_count;
    let combined_fallback = fallback_reason == 3.0
        && fallback_count > 1
        && expected_selected == f64::from(PpmlSolverFamily::Cmg.code())
        && checkpoint == f64::from(PpmlSolverFamily::Lsmr.code())
        && shadow.solver[3] == f64::from(PpmlSolverFamily::Map.code())
        && shadow.solver[4] == 1.0
        && exact_shadow_count(shadow.solver[10], "PPML checkpoint fallback count")? > 0
        && exact_shadow_count(shadow.solver[cmg_start + 10], "PPML CMG fallback count")? > 0;
    if !selected_engine_valid
        || !checkpoint_valid
        || !(no_fallback || cmg_fallback || checkpoint_fallback || combined_fallback)
    {
        return Err(internal(
            "PPML solver evidence is inconsistent with the request",
        ));
    }
    Ok(())
}

fn validate_bounded_evidence(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
) -> Result<()> {
    let requested_vce = match action.vce {
        PpmlVceRequest::Unadjusted => 1.0,
        PpmlVceRequest::Robust => 2.0,
        PpmlVceRequest::Clustered => 3.0,
    };
    let effective_vce = shadow.inference[1];
    let correction_authority = match effective_vce as u8 {
        2 => exact_shadow_count(shadow.dof[0], "PPML effective observations")?,
        3 => exact_shadow_count(shadow.dof[6], "PPML minimum cluster groups")?,
        _ => {
            return Err(internal(
                "PPML inference evidence has an invalid effective VCE",
            ))
        }
    };
    if correction_authority < 2 {
        return Err(internal(
            "PPML inference correction lacks two observations or groups",
        ));
    }
    let expected_correction = correction_authority as f64 / (correction_authority - 1) as f64;
    let tolerance = shadow.inference[14];
    let psd_state = shadow.inference[5];
    let psd_adjusted = shadow.inference[6];
    let clipped = exact_shadow_count(shadow.inference[7], "PPML clipped PSD eigenvalues")?;
    let final_rank = exact_shadow_count(shadow.inference[10], "PPML final PSD rank")?;
    let identified_rank = shadow
        .coefficient_status
        .chunks_exact(COEFFICIENT_STATUS_COLUMNS)
        .take(shadow.coefficient_count)
        .filter(|row| row[1] == 1.0)
        .count();
    if shadow.inference[0] != requested_vce
        || effective_vce != shadow.receipt[3]
        || shadow.inference[2] <= 0.0
        || relative_shadow_error(shadow.inference[2], expected_correction) > 16.0 * f64::EPSILON
        || shadow.inference[3] != if effective_vce == 2.0 { 1.0 } else { 2.0 }
        || shadow.inference[4] != 1.0
        || !matches!(psd_state, 1.0 | 2.0 | 3.0)
        || !matches!(psd_adjusted, 0.0 | 1.0)
        || !tolerance.is_finite()
        || tolerance <= 0.0
        || shadow.inference[11..14]
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0 || *value > tolerance)
        || shadow.inference[15] != 1.0
        || final_rank > identified_rank
        || clipped > identified_rank
        || psd_adjusted != f64::from(psd_state == 3.0)
        || psd_adjusted != f64::from(clipped > 0)
        || (psd_state == 1.0 && clipped != 0)
        || (psd_state == 2.0 && (shadow.inference[8] < 0.0 || clipped != 0))
        || (psd_state == 3.0 && (shadow.inference[8] >= 0.0 || clipped == 0))
    {
        return Err(internal(
            "PPML inference evidence disagrees with the request or certified result",
        ));
    }

    let mut observed_drop_counts = [0_usize; DROP_COUNT_ROWS];
    for reason in shadow.drop_reasons {
        let slot = usize::from(reason.code());
        let count = observed_drop_counts
            .get_mut(slot)
            .ok_or_else(|| internal("PPML shadow has an unknown drop reason"))?;
        *count = count
            .checked_add(1)
            .ok_or_else(|| internal("PPML shadow drop-reason count overflows"))?;
    }
    let mut published_drop_total = 0_usize;
    for (reason, expected) in observed_drop_counts.into_iter().enumerate() {
        let start = reason * DROP_COUNT_COLUMNS;
        let published_reason =
            exact_shadow_count(shadow.drop_counts[start], "PPML published drop-reason code")?;
        let published_count = exact_shadow_count(
            shadow.drop_counts[start + 1],
            "PPML published drop-reason count",
        )?;
        published_drop_total = published_drop_total
            .checked_add(published_count)
            .ok_or_else(|| internal("PPML published drop-reason total overflows"))?;
        if published_reason != reason || published_count != expected {
            return Err(internal(
                "PPML drop-count evidence disagrees with the original-row ledger",
            ));
        }
    }
    let singleton_count = exact_shadow_count(
        shadow.drop_counts
            [usize::from(fereg_core::DropReason::Singleton.code()) * DROP_COUNT_COLUMNS + 1],
        "PPML singleton drop count",
    )?;
    let separated_count = (usize::from(fereg_core::DropReason::FixedEffectSeparation.code())
        ..=usize::from(fereg_core::DropReason::MuSeparation.code()))
        .try_fold(0_usize, |total, reason| {
            total
                .checked_add(exact_shadow_count(
                    shadow.drop_counts[reason * DROP_COUNT_COLUMNS + 1],
                    "PPML separated drop count",
                )?)
                .ok_or_else(|| internal("PPML separated drop-count total overflows"))
        })?;
    if published_drop_total != shadow.drop_reasons.len()
        || shadow.drop_counts[1] != shadow.receipt[8]
        || shadow.drop_counts[DROP_COUNT_COLUMNS + 1] != shadow.receipt[6] - shadow.receipt[7]
        || separated_count != exact_shadow_count(shadow.receipt[15], "PPML separated count")?
    {
        return Err(internal(
            "PPML drop-count evidence disagrees with the result receipt",
        ));
    }

    let requested = [
        action.separation.drop_singletons,
        action.separation.methods.fixed_effects,
        action.separation.methods.simplex,
        action.separation.methods.iterated_rectifier,
        action.separation.methods.mu,
    ];
    let mut total_executions = 0_usize;
    let mut total_direct_separation = 0_usize;
    let mut total_singletons = 0_usize;
    for (row, requested) in requested.into_iter().enumerate() {
        let start = row * SEPARATION_SEQUENCE_COLUMNS;
        let method = exact_shadow_count(
            shadow.separation_sequence[start],
            "PPML sequence method code",
        )?;
        let executions = exact_shadow_count(
            shadow.separation_sequence[start + 3],
            "PPML sequence execution count",
        )?;
        let first_trace = exact_shadow_count(
            shadow.separation_sequence[start + 4],
            "PPML sequence first trace row",
        )?;
        let last_trace = exact_shadow_count(
            shadow.separation_sequence[start + 5],
            "PPML sequence last trace row",
        )?;
        let status_mask = exact_shadow_count(
            shadow.separation_sequence[start + 6],
            "PPML sequence status mask",
        )?;
        let iterations = exact_shadow_count(
            shadow.separation_sequence[start + 7],
            "PPML sequence iterations",
        )?;
        let direct_drops = exact_shadow_count(
            shadow.separation_sequence[start + 8],
            "PPML sequence direct drops",
        )?;
        let singleton_drops = exact_shadow_count(
            shadow.separation_sequence[start + 9],
            "PPML sequence singleton drops",
        )?;
        let first_generation = exact_shadow_count(
            shadow.separation_sequence[start + 10],
            "PPML sequence first generation",
        )?;
        let last_generation = exact_shadow_count(
            shadow.separation_sequence[start + 11],
            "PPML sequence last generation",
        )?;
        total_executions = total_executions
            .checked_add(executions)
            .ok_or_else(|| internal("PPML sequence execution total overflows"))?;
        if row > 0 {
            total_direct_separation = total_direct_separation
                .checked_add(direct_drops)
                .ok_or_else(|| internal("PPML sequence direct-drop total overflows"))?;
        }
        total_singletons = total_singletons
            .checked_add(singleton_drops)
            .ok_or_else(|| internal("PPML sequence singleton-drop total overflows"))?;
        let expected_order = if row == 0 || !requested { 0 } else { row };
        let aggregate = shadow
            .separation
            .chunks_exact(SEPARATION_COLUMNS)
            .find(|aggregate| aggregate[0] == (row + 1) as f64);
        let expected_aggregate = row == 0 || requested;
        let worst_status = if status_mask == 0 {
            1.0
        } else {
            f64::from(usize::BITS - status_mask.leading_zeros())
        };
        if method != row + 1
            || shadow.separation_sequence[start + 1] != f64::from(requested)
            || shadow.separation_sequence[start + 2] != expected_order as f64
            || !matches!(shadow.separation_sequence[start + 12], 0.0 | 1.0)
            || aggregate.is_some() != expected_aggregate
            || (executions == 0
                && (first_trace != 0
                    || last_trace != 0
                    || status_mask != 0
                    || iterations != 0
                    || direct_drops != 0
                    || singleton_drops != 0
                    || first_generation != 0
                    || last_generation != 0
                    || shadow.separation_sequence[start + 12] != 1.0))
            || (executions > 0
                && (!requested
                    || first_trace == 0
                    || first_trace > last_trace
                    || last_trace > shadow.separation_execution_count
                    || !(1..=127).contains(&status_mask)
                    || first_generation > last_generation
                    || last_generation > shadow.separation_generation_count))
            || aggregate.is_some_and(|aggregate| {
                aggregate[1] != worst_status
                    || aggregate[2] != executions as f64
                    || aggregate[3] != last_generation as f64
                    || aggregate[4] != iterations as f64
                    || aggregate[5] != direct_drops as f64
                    || aggregate[6] != singleton_drops as f64
                    || aggregate[7] != shadow.separation_sequence[start + 12]
            })
        {
            return Err(internal(
                "PPML separation-sequence evidence is structurally inconsistent",
            ));
        }
    }
    if total_executions != shadow.separation_execution_count
        || total_direct_separation != separated_count
        || total_singletons != singleton_count
    {
        return Err(internal(
            "PPML separation-sequence totals disagree with the bounded ledger evidence",
        ));
    }
    Ok(())
}

fn relative_shadow_error(left: f64, right: f64) -> f64 {
    if !left.is_finite() || !right.is_finite() {
        f64::INFINITY
    } else {
        (left - right).abs() / left.abs().max(right.abs()).max(1.0)
    }
}

fn exact_shadow_count(value: f64, label: &'static str) -> Result<usize> {
    const MAX_EXACT_F64_INTEGER: f64 = 9_007_199_254_740_992.0;
    if !(0.0..=MAX_EXACT_F64_INTEGER).contains(&value)
        || value.fract() != 0.0
        || value > usize::MAX as f64
    {
        Err(internal(format!("{label} is not an exact usize")))
    } else {
        Ok(value as usize)
    }
}

fn shadow_hash(high: f64, low: f64) -> Result<u64> {
    let high = exact_shadow_count(high, "PPML receipt hash high limb")?;
    let low = exact_shadow_count(low, "PPML receipt hash low limb")?;
    let high =
        u32::try_from(high).map_err(|_| internal("PPML receipt hash high limb exceeds uint32"))?;
    let low =
        u32::try_from(low).map_err(|_| internal("PPML receipt hash low limb exceeds uint32"))?;
    Ok((u64::from(high) << 32) | u64::from(low))
}

fn validate_result_request_identity(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
) -> Result<()> {
    let expected_dimension = action
        .request
        .model
        .terms
        .len()
        .checked_add(usize::from(action.request.include_intercept))
        .ok_or_else(|| allocation("PPML request coefficient dimension overflows"))?;
    let requested_threads = action
        .linear
        .execution
        .threads
        .requested
        .map_or(0, |value| value.get());
    if shadow.coefficient_count != expected_dimension
        || shadow.requested_vce != action.vce
        || shadow.adjustment_kind != action.adjustment.kind()
        || shadow.requested_memory != action.memory.requested
        || shadow.requested_threads != requested_threads
        || shadow.effective_threads != action.linear.execution.threads.resolved()
        || shadow.planned_poolsize
            != action
                .linear
                .execution
                .poolsize
                .min(shadow.planned_preflight_finite_columns)
        || shadow.planned_observation_columns != action.requested_observation_columns
        || shadow.planned_publication_cells != action.publication_matrix_cells
        || shadow.receipt[3]
            != match action.vce {
                PpmlVceRequest::Unadjusted | PpmlVceRequest::Robust => 2.0,
                PpmlVceRequest::Clustered => 3.0,
            }
        || shadow.receipt[18]
            != match action.adjustment {
                AdjustmentBinding::None => 0.0,
                AdjustmentBinding::Offset(_) => 1.0,
                AdjustmentBinding::Exposure(_) => 2.0,
            }
    {
        return Err(internal(
            "certified PPML result does not match the decoded request identity",
        ));
    }
    validate_fixed_effect_request_identity(action, shadow)?;
    validate_cluster_request_identity(action, shadow)?;
    validate_coefficient_request_identity(action, shadow)
}

fn validate_coefficient_request_identity(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
) -> Result<()> {
    for term in &action.request.model.terms {
        let slot = usize::try_from(term.coefficient_slot.get() - 1)
            .map_err(|_| internal("PPML coefficient slot does not fit usize"))?;
        let status = shadow
            .coefficient_status
            .get(slot * COEFFICIENT_STATUS_COLUMNS + 1)
            .copied()
            .ok_or_else(|| internal("certified PPML coefficient status is out of range"))?;
        let expected_omitted = if term
            .components
            .iter()
            .any(|component| component.base == MetadataFlag::True)
        {
            Some(2.0)
        } else if term.whole_column_omitted {
            Some(3.0)
        } else {
            None
        };
        if expected_omitted.is_some_and(|expected| status != expected)
            // An originally active factor slot can disappear when the
            // prepared final sample is rebased (for example, its only level
            // occurs on an observation with a missing outcome).  Core keeps
            // the original coefficient axis and records that dynamic final-
            // sample omission as SyntaxOmitted.  Original base and explicit
            // omission recipes above remain exact and cannot change status.
            || expected_omitted.is_none() && !matches!(status as u8, 1 | 3 | 4 | 5 | 6)
        {
            return Err(internal(
                "certified PPML coefficient status disagrees with the model slot recipe",
            ));
        }
    }
    let constant_slot = action.request.model.terms.len();
    let constant_status = shadow.coefficient_status[constant_slot * COEFFICIENT_STATUS_COLUMNS + 1];
    let expected_constant = if action
        .request
        .absorbed_terms
        .iter()
        .any(|term| term.intercept)
    {
        7.0
    } else {
        1.0
    };
    if constant_status != expected_constant {
        return Err(internal(
            "certified PPML structural-constant status disagrees with the absorb model",
        ));
    }
    Ok(())
}

fn validate_fixed_effect_request_identity(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
) -> Result<()> {
    let mut channel = 0_usize;
    for term in &action.request.absorbed_terms {
        if term.intercept {
            let effect = shadow
                .fixed_effect_metadata
                .get(channel)
                .ok_or_else(|| internal("certified PPML result lacks an absorbed intercept"))?;
            let direct = term.categorical_sources.len() == 1;
            let saved_requested = !term.saved_name.is_empty();
            if effect.declared_channel != channel
                || effect.term_id != term.id.get()
                || effect.kind != PpmlFixedEffectChannelKind::Intercept
                || effect.slope_source_id.is_some()
                || effect.source_ids.len() != term.categorical_sources.len()
                || effect
                    .source_ids
                    .iter()
                    .zip(&term.categorical_sources)
                    .any(|(actual, expected)| *actual != expected.get())
                || effect.saved_channel.is_some() != saved_requested
                || shadow.fixed_effects[channel * FE_COLUMNS] != if direct { 1.0 } else { 3.0 }
                || shadow.fixed_effects[channel * FE_COLUMNS + 1] != f64::from(term.id.get())
                || shadow.fixed_effects[channel * FE_COLUMNS + 2]
                    != if direct {
                        f64::from(term.categorical_sources[0].get())
                    } else {
                        0.0
                    }
            {
                return Err(internal(
                    "certified PPML absorbed-intercept identity disagrees with the request",
                ));
            }
            channel += 1;
        }
        for slope in &term.slope_sources {
            let effect = shadow
                .fixed_effect_metadata
                .get(channel)
                .ok_or_else(|| internal("certified PPML result lacks an absorbed slope"))?;
            let saved_requested = !term.saved_name.is_empty();
            if effect.declared_channel != channel
                || effect.term_id != term.id.get()
                || effect.kind != PpmlFixedEffectChannelKind::Slope
                || effect.slope_source_id != Some(slope.get())
                || effect.source_ids.len() != term.categorical_sources.len()
                || effect
                    .source_ids
                    .iter()
                    .zip(&term.categorical_sources)
                    .any(|(actual, expected)| *actual != expected.get())
                || effect.saved_channel.is_some() != saved_requested
                || shadow.fixed_effects[channel * FE_COLUMNS] != 4.0
                || shadow.fixed_effects[channel * FE_COLUMNS + 1] != f64::from(term.id.get())
                || shadow.fixed_effects[channel * FE_COLUMNS + 2] != f64::from(slope.get())
            {
                return Err(internal(
                    "certified PPML absorbed-slope identity disagrees with the request",
                ));
            }
            channel += 1;
        }
    }
    if channel != shadow.fixed_effect_metadata.len() {
        return Err(internal(
            "certified PPML fixed-effect registry has trailing channels",
        ));
    }
    Ok(())
}

fn validate_cluster_request_identity(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
) -> Result<()> {
    if action.clusters.len() != shadow.cluster_metadata.len() {
        return Err(internal(
            "certified PPML cluster registry has the wrong dimension count",
        ));
    }
    for (dimension, (requested, actual)) in action
        .clusters
        .iter()
        .zip(shadow.cluster_metadata)
        .enumerate()
    {
        let start = dimension * CLUSTER_COLUMNS;
        if actual.declared_dimension != dimension
            || actual.label != requested.label
            || actual.source_ids.len() != requested.sources.len()
            || actual
                .source_ids
                .iter()
                .zip(&requested.sources)
                .any(|(actual, expected)| *actual != expected.get())
            || shadow.clusters[start] != (dimension + 1) as f64
            || shadow.clusters[start + 1] != requested.sources.len() as f64
            || shadow.clusters[start + 2] != actual.groups as f64
            || shadow.clusters[start + 3] != 0.0
        {
            return Err(internal(
                "certified PPML cluster identity disagrees with the request",
            ));
        }
    }
    Ok(())
}

fn store_shadow_prefix(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    let mut retained_cursor = 0_usize;
    for (zero_based, reason) in shadow.drop_reasons.iter().copied().enumerate() {
        let retained = shadow
            .retained_original_rows
            .get(retained_cursor)
            .is_some_and(|row| *row == zero_based);
        spi.store_numeric(action.outputs.sample, zero_based + 1, f64::from(retained))?;
        if retained {
            for (position, values) in [
                (action.outputs.residual, shadow.residuals),
                (action.outputs.xb, shadow.xb),
                (action.outputs.absorbed, shadow.absorbed),
                (action.outputs.eta, shadow.eta),
                (action.outputs.mean, shadow.mean),
            ] {
                if let Some(position) = position {
                    spi.store_numeric(position, zero_based + 1, values[retained_cursor])?;
                }
            }
            for (position, values) in action
                .outputs
                .saved_channels
                .iter()
                .zip(&shadow.saved_channels)
            {
                if let Some(position) = *position {
                    spi.store_numeric(position, zero_based + 1, values[retained_cursor])?;
                }
            }
            retained_cursor += 1;
        }
        if let Some(position) = action.outputs.drop_reason {
            spi.store_numeric(position, zero_based + 1, f64::from(reason.code()))?;
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
    store_matrix(spi, &action.matrices.fit, FIT_ROWS, 1, &shadow.fit)?;
    store_matrix(spi, &action.matrices.dof, DOF_ROWS, 1, &shadow.dof)?;
    store_matrix(
        spi,
        &action.matrices.fixed_effects,
        shadow.fixed_effect_rows.max(1),
        FE_COLUMNS,
        &shadow.fixed_effects,
    )?;
    store_matrix(
        spi,
        &action.matrices.clusters,
        shadow.cluster_rows.max(1),
        CLUSTER_COLUMNS,
        &shadow.clusters,
    )?;
    store_matrix(
        spi,
        &action.matrices.subsets,
        shadow.subset_rows.max(1),
        SUBSET_COLUMNS,
        &shadow.subsets,
    )?;
    store_matrix(
        spi,
        &action.matrices.separation,
        shadow.separation_rows.max(1),
        SEPARATION_COLUMNS,
        &shadow.separation,
    )?;
    store_matrix(
        spi,
        &action.matrices.inference,
        INFERENCE_ROWS,
        1,
        &shadow.inference,
    )?;
    store_matrix(
        spi,
        &action.matrices.drop_counts,
        DROP_COUNT_ROWS,
        DROP_COUNT_COLUMNS,
        &shadow.drop_counts,
    )?;
    store_matrix(
        spi,
        &action.matrices.separation_sequence,
        SEPARATION_SEQUENCE_ROWS,
        SEPARATION_SEQUENCE_COLUMNS,
        &shadow.separation_sequence,
    )?;
    store_matrix(
        spi,
        &action.matrices.certificates,
        CERTIFICATE_ROWS,
        1,
        &shadow.certificates,
    )?;
    store_matrix(
        spi,
        &action.matrices.solver,
        SOLVER_ROWS,
        SOLVER_COLUMNS,
        &shadow.solver,
    )?;
    for column in 1..PPML_RESULT_RECEIPT_COMPLETE_COLUMN {
        spi.store_matrix(
            &action.matrices.receipt,
            1,
            column,
            shadow.receipt[column - 1],
        )?;
    }
    Ok(())
}

fn store_runtime_and_complete(
    action: &EstimatePpmlAction,
    shadow: &PpmlShadowResult<'_>,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    store_matrix(
        spi,
        &action.matrices.runtime,
        RUNTIME_ROWS,
        1,
        &shadow.runtime,
    )?;
    // Receipt-last is the transaction boundary. No SPI call, allocation,
    // interrupt poll, or scientific computation may follow this store.
    spi.store_matrix(
        &action.matrices.receipt,
        1,
        PPML_RESULT_RECEIPT_COMPLETE_COLUMN,
        PPML_RESULT_RECEIPT_COMPLETE_VALUE,
    )
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

fn validate_ppml_weights(request: &OlsRequestV1) -> Result<()> {
    if request
        .bindings
        .iter()
        .any(|binding| binding.role == OlsInputRole::AnalyticWeight)
    {
        return Err(unsupported("PPML supports only fweights and pweights"));
    }
    Ok(())
}

fn parse_ppml_vce(value: &str, clusters: &[OlsClusterTermV1]) -> Result<PpmlVceRequest> {
    match (value, clusters.is_empty()) {
        ("robust", true) => Ok(PpmlVceRequest::Robust),
        ("unadjusted" | "iid", true) => Ok(PpmlVceRequest::Unadjusted),
        ("cluster" | "clustered", false) => Ok(PpmlVceRequest::Clustered),
        ("cluster" | "clustered", true) => Err(invalid(
            "clustered PPML inference requires at least one cluster dimension",
        )),
        ("robust" | "unadjusted" | "iid", false) => Err(invalid(
            "nonclustered PPML inference cannot carry cluster dimensions",
        )),
        _ => Err(unsupported(format!("unsupported PPML VCE {value:?}"))),
    }
}

fn publication_matrix_cells(matrices: &PreparedResultMatrices) -> Result<usize> {
    prepared_matrix_names(matrices)
        .into_iter()
        .try_fold(0_usize, |total, matrix| {
            let cells = matrix
                .rows()
                .checked_mul(matrix.columns())
                .ok_or_else(|| allocation("PPML publication matrix cell count overflows"))?;
            total
                .checked_add(cells)
                .ok_or_else(|| allocation("PPML publication matrix cell count overflows"))
        })
}

fn read_ppml_clusters(
    spec: &EstimatePpmlSpec,
    spi: &mut dyn SpiApi,
    request: &OlsRequestV1,
) -> Result<Vec<OlsClusterTermV1>> {
    let terms = read_table(
        spi,
        &spec.cluster_term_matrix,
        spec.cluster_term_count,
        CLUSTER_TERM_ROW_WIDTH,
        "PPML cluster term",
    )?;
    let components = read_table(
        spi,
        &spec.cluster_component_matrix,
        spec.cluster_component_count,
        CLUSTER_COMPONENT_ROW_WIDTH,
        "PPML cluster component",
    )?;
    if spec.cluster_term_count == 0 {
        if spec.cluster_component_count != 0
            || terms.rows[0].iter().any(|value| *value != 0.0)
            || components.rows[0].iter().any(|value| *value != 0.0)
        {
            return Err(invalid(
                "empty PPML cluster metadata requires exact zero sentinels",
            ));
        }
        return Ok(Vec::new());
    }
    if !(1..=5).contains(&spec.cluster_term_count) {
        return Err(unsupported(
            "PPML cluster metadata supports between one and five dimensions",
        ));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(spec.cluster_term_count)
        .map_err(|_| allocation("unable to reserve PPML cluster terms"))?;
    let mut cursor = 0_usize;
    for term_index in 0..spec.cluster_term_count {
        let row = &terms.rows[term_index];
        let identity = exact_usize(row[0], "PPML cluster term identity")?;
        let count = exact_usize(row[1], "PPML cluster source count")?;
        if identity != term_index + 1 || count == 0 {
            return Err(invalid("PPML cluster term order or arity is invalid"));
        }
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(count)
            .map_err(|_| allocation("unable to reserve PPML cluster source recipe"))?;
        for order in 1..=count {
            let component = components.rows.get(cursor).ok_or_else(|| {
                invalid("PPML cluster component table ended before its declared arity")
            })?;
            if exact_usize(component[0], "PPML cluster component term")? != identity
                || exact_usize(component[1], "PPML cluster component order")? != order
            {
                return Err(invalid("PPML cluster component ordering is invalid"));
            }
            let source_value = exact_usize(component[2], "PPML cluster source identity")?;
            let source = SourceId::new(
                u32::try_from(source_value)
                    .map_err(|_| invalid("PPML cluster source exceeds u32"))?,
            )?;
            if sources.contains(&source)
                || !request.model.sources.iter().any(|entry| entry.id == source)
            {
                return Err(invalid("PPML cluster source recipe is invalid"));
            }
            sources.push(source);
            cursor += 1;
        }
        output.push(OlsClusterTermV1 {
            label: fallible_string(
                &spec.cluster_labels[term_index],
                "unable to reserve PPML cluster label",
            )?,
            sources,
        });
    }
    if cursor != spec.cluster_component_count {
        return Err(invalid("PPML cluster component table has trailing rows"));
    }
    Ok(output)
}

fn cluster_subset_rows(dimensions: usize) -> Result<usize> {
    if dimensions == 0 {
        return Ok(0);
    }
    1_usize
        .checked_shl(
            u32::try_from(dimensions)
                .map_err(|_| allocation("PPML cluster dimension count exceeds u32"))?,
        )
        .and_then(|subsets| subsets.checked_sub(1))
        .ok_or_else(|| allocation("PPML cluster subset count overflows"))
}

fn validate_adjustment_binding(adjustment: AdjustmentBinding, spi: &mut dyn SpiApi) -> Result<()> {
    let Some(position) = adjustment.position() else {
        return Ok(());
    };
    let nvars = spi.nvars()?;
    if position as usize > nvars {
        return Err(invalid(
            "PPML adjustment position exceeds the plugin varlist",
        ));
    }
    if spi.variable_is_string(position)? || spi.variable_is_strl(position)? {
        return Err(unsupported("PPML adjustment input must be numeric"));
    }
    Ok(())
}

fn validate_variable_layout(
    request: &OlsRequestV1,
    adjustment: AdjustmentBinding,
    outputs: &ObservationOutputs,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    let nvars = spi.nvars()?;
    let output_count = outputs.positions().count();
    let expected = request
        .bindings
        .len()
        .checked_add(usize::from(adjustment.position().is_some()))
        .and_then(|inputs| inputs.checked_add(output_count))
        .ok_or_else(|| allocation("PPML plugin varlist width overflows"))?;
    if nvars != expected {
        return Err(invalid(format!(
            "PPML plugin varlist has {nvars} variables, expected exactly {expected}",
        )));
    }
    for (index, binding) in request.bindings.iter().enumerate() {
        let position = binding.stata_var_position;
        if position == 0 || position as usize > nvars {
            return Err(invalid("PPML input position exceeds the plugin varlist"));
        }
        if request.bindings[..index]
            .iter()
            .any(|previous| previous.stata_var_position == position)
        {
            return Err(invalid("PPML input variables alias one another"));
        }
    }
    if let Some(position) = adjustment.position() {
        if request
            .bindings
            .iter()
            .any(|binding| binding.stata_var_position == position)
        {
            return Err(invalid("PPML adjustment aliases another input"));
        }
    }
    for (index, position) in outputs.positions().enumerate() {
        if position == 0 || position as usize > nvars {
            return Err(invalid("PPML output position exceeds the plugin varlist"));
        }
        if request
            .bindings
            .iter()
            .any(|binding| binding.stata_var_position == position)
            || adjustment.position() == Some(position)
            || outputs
                .positions()
                .take(index)
                .any(|previous| previous == position)
        {
            return Err(invalid(
                "PPML output variables alias an input or another output",
            ));
        }
        if spi.variable_is_string(position)? || spi.variable_is_strl(position)? {
            return Err(unsupported("PPML output variables must be numeric"));
        }
    }
    // `nvars == expected`, every position is in range, and the positions are
    // pairwise distinct. Those three facts imply exact coverage of 1..=nvars
    // without allocating a coefficient-sized set.
    Ok(())
}

fn validate_matrix_layout(
    matrices: &ResultMatrices,
    dimensions: ResultMatrixDimensions,
    spi: &mut dyn SpiApi,
    input_matrices: &[&String],
) -> Result<PreparedResultMatrices> {
    let dimension = dimensions.coefficient_count.max(1);
    let layouts = [
        (&matrices.b, 1, dimension),
        (&matrices.covariance, dimension, dimension),
        (
            &matrices.coefficient_status,
            dimension,
            COEFFICIENT_STATUS_COLUMNS,
        ),
        (&matrices.fit, FIT_ROWS, 1),
        (&matrices.dof, DOF_ROWS, 1),
        (
            &matrices.fixed_effects,
            dimensions.fixed_effect_rows.max(1),
            FE_COLUMNS,
        ),
        (
            &matrices.clusters,
            dimensions.cluster_dimensions.max(1),
            CLUSTER_COLUMNS,
        ),
        (
            &matrices.subsets,
            dimensions.cluster_subset_rows.max(1),
            SUBSET_COLUMNS,
        ),
        (
            &matrices.separation,
            dimensions.separation_rows.max(1),
            SEPARATION_COLUMNS,
        ),
        (&matrices.inference, INFERENCE_ROWS, 1),
        (&matrices.drop_counts, DROP_COUNT_ROWS, DROP_COUNT_COLUMNS),
        (
            &matrices.separation_sequence,
            SEPARATION_SEQUENCE_ROWS,
            SEPARATION_SEQUENCE_COLUMNS,
        ),
        (&matrices.certificates, CERTIFICATE_ROWS, 1),
        (&matrices.solver, SOLVER_ROWS, SOLVER_COLUMNS),
        (&matrices.runtime, RUNTIME_ROWS, 1),
        (&matrices.receipt, 1, PPML_RESULT_RECEIPT_COLUMNS),
    ];
    let input_names = input_matrices
        .iter()
        .map(|name| name.as_str())
        .collect::<BTreeSet<_>>();
    let mut output_names = BTreeSet::new();
    for (name, rows, columns) in layouts {
        if name.is_empty() || input_names.contains(name.as_str()) || !output_names.insert(name) {
            return Err(invalid(
                "PPML result matrices alias an input or another output matrix",
            ));
        }
        if spi.matrix_dimensions(name)? != (rows, columns) {
            return Err(invalid(format!(
                "PPML output matrix {name} has an invalid layout",
            )));
        }
    }
    for column in 1..=PPML_RESULT_RECEIPT_COLUMNS {
        if spi.matrix_element(&matrices.receipt, 1, column)? != 0.0 {
            return Err(invalid(
                "PPML output receipt must be all zero before observation loading",
            ));
        }
    }
    Ok(PreparedResultMatrices {
        b: PreparedMatrixStoreName::try_new(&matrices.b, 1, dimension)?,
        covariance: PreparedMatrixStoreName::try_new(&matrices.covariance, dimension, dimension)?,
        coefficient_status: PreparedMatrixStoreName::try_new(
            &matrices.coefficient_status,
            dimension,
            COEFFICIENT_STATUS_COLUMNS,
        )?,
        fit: PreparedMatrixStoreName::try_new(&matrices.fit, FIT_ROWS, 1)?,
        dof: PreparedMatrixStoreName::try_new(&matrices.dof, DOF_ROWS, 1)?,
        fixed_effects: PreparedMatrixStoreName::try_new(
            &matrices.fixed_effects,
            dimensions.fixed_effect_rows.max(1),
            FE_COLUMNS,
        )?,
        clusters: PreparedMatrixStoreName::try_new(
            &matrices.clusters,
            dimensions.cluster_dimensions.max(1),
            CLUSTER_COLUMNS,
        )?,
        subsets: PreparedMatrixStoreName::try_new(
            &matrices.subsets,
            dimensions.cluster_subset_rows.max(1),
            SUBSET_COLUMNS,
        )?,
        separation: PreparedMatrixStoreName::try_new(
            &matrices.separation,
            dimensions.separation_rows.max(1),
            SEPARATION_COLUMNS,
        )?,
        inference: PreparedMatrixStoreName::try_new(&matrices.inference, INFERENCE_ROWS, 1)?,
        drop_counts: PreparedMatrixStoreName::try_new(
            &matrices.drop_counts,
            DROP_COUNT_ROWS,
            DROP_COUNT_COLUMNS,
        )?,
        separation_sequence: PreparedMatrixStoreName::try_new(
            &matrices.separation_sequence,
            SEPARATION_SEQUENCE_ROWS,
            SEPARATION_SEQUENCE_COLUMNS,
        )?,
        certificates: PreparedMatrixStoreName::try_new(
            &matrices.certificates,
            CERTIFICATE_ROWS,
            1,
        )?,
        solver: PreparedMatrixStoreName::try_new(&matrices.solver, SOLVER_ROWS, SOLVER_COLUMNS)?,
        runtime: PreparedMatrixStoreName::try_new(&matrices.runtime, RUNTIME_ROWS, 1)?,
        receipt: PreparedMatrixStoreName::try_new(
            &matrices.receipt,
            1,
            PPML_RESULT_RECEIPT_COLUMNS,
        )?,
    })
}

fn prepared_matrix_names(matrices: &PreparedResultMatrices) -> [&PreparedMatrixStoreName; 16] {
    [
        &matrices.b,
        &matrices.covariance,
        &matrices.coefficient_status,
        &matrices.fit,
        &matrices.dof,
        &matrices.fixed_effects,
        &matrices.clusters,
        &matrices.subsets,
        &matrices.separation,
        &matrices.inference,
        &matrices.drop_counts,
        &matrices.separation_sequence,
        &matrices.certificates,
        &matrices.solver,
        &matrices.runtime,
        &matrices.receipt,
    ]
}

fn parse_separation_methods(value: &str) -> Result<SeparationMethods> {
    if value.trim().is_empty() || value == "default" {
        return Ok(SeparationMethods::default());
    }
    if value == "none" {
        return Ok(SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: false,
            mu: false,
        });
    }
    let mut methods = SeparationMethods {
        fixed_effects: false,
        simplex: false,
        iterated_rectifier: false,
        mu: false,
    };
    let mut seen = BTreeSet::new();
    for token in value
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .filter(|token| !token.is_empty())
    {
        if !seen.insert(token) {
            return Err(syntax("PPML separation methods may not be repeated"));
        }
        match token {
            "fe" => methods.fixed_effects = true,
            "simplex" => methods.simplex = true,
            "ir" => methods.iterated_rectifier = true,
            "mu" => methods.mu = true,
            _ => return Err(syntax(format!("unknown PPML separation method {token:?}"))),
        }
    }
    Ok(methods)
}

fn separation_method_capacity(methods: SeparationMethods) -> usize {
    1 + usize::from(methods.fixed_effects)
        + usize::from(methods.simplex)
        + usize::from(methods.iterated_rectifier)
        + usize::from(methods.mu)
}

fn parse_residual_request(value: &str, output: Option<u32>) -> Result<Option<PpmlResidualKind>> {
    let kind = match value {
        "none" => None,
        "response" => Some(PpmlResidualKind::Response),
        "score" => Some(PpmlResidualKind::Score),
        "pearson" => Some(PpmlResidualKind::Pearson),
        "working" => Some(PpmlResidualKind::Working),
        "deviance" => Some(PpmlResidualKind::Deviance),
        "anscombe" => Some(PpmlResidualKind::Anscombe),
        _ => {
            return Err(syntax(
                "PPML residual must be none, response, score, pearson, working, deviance, or anscombe",
            ));
        }
    };
    if kind.is_some() != output.is_some() {
        return Err(syntax(
            "PPML residual kind and output_residual position disagree",
        ));
    }
    Ok(kind)
}

fn validate_saved_output_layout(
    request: &OlsRequestV1,
    outputs: &ObservationOutputs,
) -> Result<()> {
    let expected = request
        .absorbed_terms
        .iter()
        .try_fold(0_usize, |total, term| {
            total
                .checked_add(usize::from(term.intercept) + term.slope_sources.len())
                .ok_or_else(|| allocation("PPML saved fixed-effect channel count overflows"))
        })?;
    if outputs.saved_channels.len() != expected {
        return Err(invalid(
            "PPML saved-output registry does not match the absorbed channel count",
        ));
    }
    let mut cursor = 0_usize;
    for term in &request.absorbed_terms {
        let requested = !term.saved_name.is_empty();
        let channels = usize::from(term.intercept) + term.slope_sources.len();
        for _ in 0..channels {
            if outputs.saved_channels[cursor].is_some() != requested {
                return Err(invalid(
                    "PPML saved-output positions disagree with absorbed saved-name metadata",
                ));
            }
            cursor += 1;
        }
    }
    Ok(())
}

fn decode_output_position_registry(value: &str, expected: usize) -> Result<Vec<Option<u32>>> {
    let values = decode_ppml_registry(value, expected)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(expected)
        .map_err(|_| allocation("unable to reserve PPML saved-output registry"))?;
    for value in values {
        let position = value
            .parse::<u32>()
            .map_err(|_| syntax("PPML saved-output position must be an unsigned integer"))?;
        output.push((position != 0).then_some(position));
    }
    Ok(output)
}

fn decode_ppml_registry(value: &str, expected: usize) -> Result<Vec<String>> {
    if expected == 0 {
        if value != "." {
            return Err(invalid("empty registry must use the dot sentinel"));
        }
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(expected)
        .map_err(|_| allocation("unable to reserve PPML length-framed registry"))?;
    for piece in value.split('|') {
        if output.len() == expected {
            return Err(invalid(
                "registry cardinality does not match its declared count",
            ));
        }
        let (length, payload) = piece
            .split_once(':')
            .ok_or_else(|| invalid("registry item lacks a length prefix"))?;
        let length = length
            .parse::<usize>()
            .map_err(|_| syntax("registry length must be an unsigned integer"))?;
        if payload.len() != length {
            return Err(invalid("registry byte length is incorrect"));
        }
        output.push(fallible_string(payload, "PPML registry payload")?);
    }
    if output.len() != expected {
        return Err(invalid(
            "registry cardinality does not match its declared count",
        ));
    }
    Ok(output)
}

fn optional_position(values: &BTreeMap<String, String>, key: &str) -> Result<Option<u32>> {
    let position = parse_nonnegative_u32(values, key)?;
    Ok((position != 0).then_some(position))
}

fn exact_usize(value: f64, label: &str) -> Result<usize> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        Err(invalid(format!(
            "{label} is not an exact nonnegative integer"
        )))
    } else {
        Ok(value as usize)
    }
}

fn fallible_string(value: &str, label: &'static str) -> Result<String> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    output.push_str(value);
    Ok(output)
}

fn fallible_copy_values<T: Copy>(values: &[T], label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    output.extend_from_slice(values);
    Ok(output)
}

fn fallible_clone_component(component: &TermComponent) -> Result<TermComponent> {
    Ok(TermComponent {
        order: component.order,
        source: component.source,
        kind: component.kind,
        level_state: component.level_state,
        level_value: component.level_value,
        base: component.base,
        omitted: component.omitted,
        raw_operator: fallible_string(&component.raw_operator, "PPML component operator")?,
        time_series_state: component.time_series_state,
        raw_time_series_operator: fallible_string(
            &component.raw_time_series_operator,
            "PPML component time-series operator",
        )?,
    })
}

fn fallible_clone_model(model: &CompiledTermGraph) -> Result<CompiledTermGraph> {
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(model.sources.len())
        .map_err(|_| allocation("unable to reserve PPML request sources"))?;
    for source in &model.sources {
        sources.push(SourceEntry {
            id: source.id,
            canonical_name: fallible_string(&source.canonical_name, "PPML request source name")?,
        });
    }
    let mut terms = Vec::new();
    terms
        .try_reserve_exact(model.terms.len())
        .map_err(|_| allocation("unable to reserve PPML request terms"))?;
    for term in &model.terms {
        let mut components = Vec::new();
        components
            .try_reserve_exact(term.components.len())
            .map_err(|_| allocation("unable to reserve PPML request term components"))?;
        for component in &term.components {
            components.push(fallible_clone_component(component)?);
        }
        terms.push(ExpandedTerm {
            id: term.id,
            order: term.order,
            coefficient_slot: term.coefficient_slot,
            raw_kind: term.raw_kind,
            whole_column_omitted: term.whole_column_omitted,
            canonical_token: fallible_string(
                &term.canonical_token,
                "PPML request canonical token",
            )?,
            components,
        });
    }
    Ok(CompiledTermGraph { sources, terms })
}

fn fallible_clone_absorbed_terms(values: &[AbsorbedTerm]) -> Result<Vec<AbsorbedTerm>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| allocation("unable to reserve PPML absorbed-term request"))?;
    for term in values {
        output.push(AbsorbedTerm {
            id: term.id,
            intercept: term.intercept,
            categorical_sources: fallible_copy_values(
                &term.categorical_sources,
                "PPML absorbed categorical sources",
            )?,
            slope_sources: fallible_copy_values(
                &term.slope_sources,
                "PPML absorbed slope sources",
            )?,
            label: fallible_string(&term.label, "PPML absorbed label")?,
            saved_name: fallible_string(&term.saved_name, "PPML absorbed saved name")?,
        });
    }
    Ok(output)
}

fn fallible_clone_ols_request(request: &OlsRequestV1) -> Result<OlsRequestV1> {
    Ok(OlsRequestV1 {
        model: fallible_clone_model(&request.model)?,
        absorbed_terms: fallible_clone_absorbed_terms(&request.absorbed_terms)?,
        bindings: fallible_copy_values(&request.bindings, "PPML request bindings")?,
        include_intercept: request.include_intercept,
        keep_singletons: request.keep_singletons,
    })
}

fn fallible_clone_clusters(values: &[OlsClusterTermV1]) -> Result<Vec<OlsClusterTermV1>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| allocation("unable to reserve PPML cluster request"))?;
    for cluster in values {
        output.push(OlsClusterTermV1 {
            label: fallible_string(&cluster.label, "PPML cluster label")?,
            sources: fallible_copy_values(&cluster.sources, "PPML cluster sources")?,
        });
    }
    Ok(output)
}

fn reserved_filled<T: Clone>(elements: usize, value: T, label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    output.resize(elements, value);
    Ok(output)
}

fn syntax(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Syntax, message)
}

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
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

fn internal(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fereg_core::metadata::{
        AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledModelSpecification, CompiledTermGraph,
        ComponentKind, ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, RawTermKind,
        SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
    };
    use fereg_core::protocol::{ADO_VERSION, MODEL_WIRE_VERSION};
    use std::collections::BTreeMap;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    #[cfg(feature = "ffi")]
    use std::sync::atomic::{AtomicU64, Ordering};

    #[cfg(feature = "ffi")]
    static EARLY_PREFLIGHT_CURRENT_BYTES: AtomicU64 = AtomicU64::new(0);
    #[cfg(feature = "ffi")]
    static EARLY_PREFLIGHT_PEAK_BYTES: AtomicU64 = AtomicU64::new(0);
    #[cfg(feature = "ffi")]
    static EARLY_PREFLIGHT_EVENTS: AtomicU64 = AtomicU64::new(0);

    #[cfg(feature = "ffi")]
    fn record_early_preflight_allocator(
        event: fereg_core::ppml::PpmlMemoryPreflightEvent,
    ) -> Result<()> {
        if matches!(
            event,
            fereg_core::ppml::PpmlMemoryPreflightEvent::Decision(_)
        ) {
            let snapshot = crate::ffi::allocation_usage();
            EARLY_PREFLIGHT_CURRENT_BYTES.store(snapshot.current_bytes, Ordering::Relaxed);
            EARLY_PREFLIGHT_PEAK_BYTES.store(snapshot.peak_bytes, Ordering::Relaxed);
            EARLY_PREFLIGHT_EVENTS.store(snapshot.events, Ordering::Relaxed);
        }
        Ok(())
    }

    #[derive(Debug, Clone, PartialEq)]
    enum StoreEvent {
        Numeric(u32, usize, f64),
        Matrix(String, usize, usize, f64),
    }

    struct FakeSpi {
        local_bundle: Option<Vec<u8>>,
        macro_reads: usize,
        nobs: usize,
        nvars: usize,
        numeric: BTreeMap<(u32, usize), (f64, bool)>,
        matrices: BTreeMap<String, Vec<Vec<f64>>>,
        reads: usize,
        stores: Vec<StoreEvent>,
        store_calls: usize,
        fail_store_at: Option<usize>,
        interrupt_at: Option<usize>,
        interrupt_checks: usize,
        interrupt_after_display: bool,
        interrupt_when_marker_exists: Option<std::path::PathBuf>,
        displays: Vec<String>,
    }

    impl FakeSpi {
        fn receipt_complete(&self) -> f64 {
            self.matrices["RREC"][0][PPML_RESULT_RECEIPT_COMPLETE_COLUMN - 1]
        }

        fn output(&self, variable: u32, observation: usize) -> Option<f64> {
            self.numeric
                .get(&(variable, observation))
                .map(|(value, _)| *value)
        }
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

        fn variable_is_string(&mut self, _variable: u32) -> Result<bool> {
            Ok(false)
        }

        fn variable_is_strl(&mut self, _variable: u32) -> Result<bool> {
            Ok(false)
        }

        fn read_numeric(&mut self, variable: u32, observation: usize) -> Result<(f64, bool)> {
            self.reads += 1;
            self.numeric
                .get(&(variable, observation))
                .copied()
                .ok_or_else(|| invalid("fake PPML numeric input is missing"))
        }

        fn read_string(&mut self, _variable: u32, _observation: usize) -> Result<Vec<u8>> {
            Err(unsupported("fake PPML fixture has no string variables"))
        }

        fn matrix_dimensions(&mut self, name: &str) -> Result<(usize, usize)> {
            let matrix = self
                .matrices
                .get(name)
                .ok_or_else(|| invalid("fake PPML matrix is missing"))?;
            Ok((matrix.len(), matrix.first().map(Vec::len).unwrap_or(0)))
        }

        fn matrix_element(&mut self, name: &str, row: usize, column: usize) -> Result<f64> {
            self.matrices
                .get(name)
                .and_then(|matrix| matrix.get(row - 1))
                .and_then(|row| row.get(column - 1))
                .copied()
                .ok_or_else(|| invalid("fake PPML matrix coordinate is missing"))
        }

        fn store_numeric(&mut self, variable: u32, observation: usize, value: f64) -> Result<()> {
            self.store_calls += 1;
            if self.fail_store_at == Some(self.store_calls) {
                return Err(internal("injected PPML numeric-store failure"));
            }
            self.numeric.insert((variable, observation), (value, false));
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
            self.store_calls += 1;
            if self.fail_store_at == Some(self.store_calls) {
                return Err(internal("injected PPML matrix-store failure"));
            }
            self.matrices
                .get_mut(name.as_str())
                .and_then(|matrix| matrix.get_mut(row - 1))
                .and_then(|row| row.get_mut(column - 1))
                .map(|slot| *slot = value)
                .ok_or_else(|| invalid("fake PPML matrix store is out of range"))?;
            self.stores.push(StoreEvent::Matrix(
                name.as_str().to_owned(),
                row,
                column,
                value,
            ));
            Ok(())
        }

        fn display(&mut self, message: &str) -> Result<()> {
            self.displays.push(message.to_owned());
            if self.interrupt_after_display {
                self.interrupt_at = Some(self.interrupt_checks.saturating_add(1));
            }
            Ok(())
        }

        fn check_interrupted(&mut self) -> Result<()> {
            self.interrupt_checks += 1;
            if self.interrupt_at == Some(self.interrupt_checks)
                || self
                    .interrupt_when_marker_exists
                    .as_ref()
                    .is_some_and(|path| path.exists())
            {
                Err(FeregError::new(
                    ErrorCode::Interrupted,
                    "injected hidden PPML interruption",
                ))
            } else {
                Ok(())
            }
        }
    }

    fn fixture() -> (Vec<String>, FakeSpi) {
        let source = SourceId::new(1).expect("source");
        let graph = CompiledTermGraph {
            sources: vec![SourceEntry {
                id: source,
                canonical_name: "x".to_owned(),
            }],
            terms: vec![ExpandedTerm {
                id: TermId::new(1).expect("term"),
                order: TermOrder::new(1).expect("order"),
                coefficient_slot: CoefficientSlot::new(1).expect("slot"),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: "c.x".to_owned(),
                components: vec![TermComponent {
                    order: ComponentOrder::new(1).expect("component"),
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
            }],
        };
        let wire = ModelWireV2::from_graph(&graph).expect("wire");
        let mut matrices = BTreeMap::from([
            ("S".to_owned(), wire.sources.rows),
            ("T".to_owned(), wire.terms.rows),
            ("C".to_owned(), wire.components.rows),
            ("A".to_owned(), wire.absorb_terms.rows),
            ("AC".to_owned(), wire.absorb_components.rows),
            ("CT".to_owned(), zero_matrix(1, CLUSTER_TERM_ROW_WIDTH)),
            ("CC".to_owned(), zero_matrix(1, CLUSTER_COMPONENT_ROW_WIDTH)),
            (
                "BIND".to_owned(),
                vec![
                    vec![1.0, 1.0, 0.0],
                    vec![2.0, 2.0, 0.0],
                    vec![3.0, 3.0, 1.0],
                ],
            ),
            ("RB".to_owned(), zero_matrix(1, 2)),
            ("RV".to_owned(), zero_matrix(2, 2)),
            (
                "RSTATUS".to_owned(),
                zero_matrix(2, COEFFICIENT_STATUS_COLUMNS),
            ),
            ("RFIT".to_owned(), zero_matrix(FIT_ROWS, 1)),
            ("RDOF".to_owned(), zero_matrix(DOF_ROWS, 1)),
            ("RFE".to_owned(), zero_matrix(1, FE_COLUMNS)),
            ("RCLUSTER".to_owned(), zero_matrix(1, CLUSTER_COLUMNS)),
            ("RSUBSET".to_owned(), zero_matrix(1, SUBSET_COLUMNS)),
            (
                "RSEP".to_owned(),
                zero_matrix(
                    separation_method_capacity(SeparationMethods::default()),
                    SEPARATION_COLUMNS,
                ),
            ),
            ("RINF".to_owned(), zero_matrix(INFERENCE_ROWS, 1)),
            (
                "RDROP".to_owned(),
                zero_matrix(DROP_COUNT_ROWS, DROP_COUNT_COLUMNS),
            ),
            (
                "RSEQ".to_owned(),
                zero_matrix(SEPARATION_SEQUENCE_ROWS, SEPARATION_SEQUENCE_COLUMNS),
            ),
            ("RCERT".to_owned(), zero_matrix(CERTIFICATE_ROWS, 1)),
            (
                "RSOLVER".to_owned(),
                zero_matrix(SOLVER_ROWS, SOLVER_COLUMNS),
            ),
            ("RRUNTIME".to_owned(), zero_matrix(RUNTIME_ROWS, 1)),
            (
                "RREC".to_owned(),
                zero_matrix(1, PPML_RESULT_RECEIPT_COLUMNS),
            ),
        ]);
        let mut numeric = BTreeMap::new();
        for row in 0..10 {
            let x = row as f64 / 4.0;
            let outcome = (0.2 + 0.15 * x).exp();
            let observation = row + 1;
            numeric.insert((1, observation), (1.0, false));
            numeric.insert((2, observation), (outcome, false));
            numeric.insert((3, observation), (x, false));
            for variable in 4..=10 {
                numeric.insert((variable, observation), (-999.0, false));
            }
        }
        let args = vec![
            format!("ado={ADO_VERSION}"),
            format!("protocol_major={}", ACTION_PROTOCOL_VERSION.major),
            format!("protocol_minor={}", ACTION_PROTOCOL_VERSION.minor),
            "stata=18.0".to_owned(),
            format!("confirm={CONFIRMATION}"),
            format!("plugin={PLUGIN_VERSION}"),
            format!("request_major={}", PPML_REQUEST_WIRE_VERSION.major),
            format!("request_minor={}", PPML_REQUEST_WIRE_VERSION.minor),
            format!("result_major={}", PPML_RESULT_WIRE_VERSION.major),
            format!("result_minor={}", PPML_RESULT_WIRE_VERSION.minor),
            format!("model_major={}", MODEL_WIRE_VERSION.major),
            format!("model_minor={}", MODEL_WIRE_VERSION.minor),
            "source_matrix=S".to_owned(),
            "source_count=1".to_owned(),
            "term_matrix=T".to_owned(),
            "term_count=1".to_owned(),
            "component_matrix=C".to_owned(),
            "component_count=1".to_owned(),
            format!("source_names={}", registry(&wire.source_names)),
            format!("canonical_tokens={}", registry(&wire.canonical_tokens)),
            format!(
                "raw_component_operators={}",
                registry(&wire.raw_component_operators),
            ),
            format!(
                "raw_time_series_operators={}",
                registry(&wire.raw_time_series_operators),
            ),
            "absorb_term_matrix=A".to_owned(),
            "absorb_term_count=0".to_owned(),
            "absorb_component_matrix=AC".to_owned(),
            "absorb_component_count=0".to_owned(),
            "absorb_labels=.".to_owned(),
            "absorb_saved_names=.".to_owned(),
            "cluster_term_matrix=CT".to_owned(),
            "cluster_term_count=0".to_owned(),
            "cluster_component_matrix=CC".to_owned(),
            "cluster_component_count=0".to_owned(),
            "cluster_labels=.".to_owned(),
            "binding_matrix=BIND".to_owned(),
            "binding_count=3".to_owned(),
            "include_intercept=1".to_owned(),
            "candidate_all=1".to_owned(),
            "input_complete=1".to_owned(),
            "keep_singletons=0".to_owned(),
            "adjustment=none".to_owned(),
            "adjustment_position=0".to_owned(),
            "residual=response".to_owned(),
            "vce=robust".to_owned(),
            "memory=auto".to_owned(),
            "poolsize=10".to_owned(),
            "threads=0".to_owned(),
            "deterministic=0".to_owned(),
            "solver=auto".to_owned(),
            "tolerance=1e-8".to_owned(),
            "iterate=10000".to_owned(),
            "separation=default".to_owned(),
            "separation_tolerance=1e-7".to_owned(),
            "separation_max_iterations=100".to_owned(),
            "simplex_max_iterations=10000".to_owned(),
            "simplex_max_tableau_cells=8000000".to_owned(),
            "output_sample=4".to_owned(),
            "output_residual=5".to_owned(),
            "output_xb=9".to_owned(),
            "output_absorbed=10".to_owned(),
            "output_eta=6".to_owned(),
            "output_mean=7".to_owned(),
            "output_drop_reason=8".to_owned(),
            "output_saved_count=0".to_owned(),
            "output_saved_channels=.".to_owned(),
            "output_b_matrix=RB".to_owned(),
            "output_v_matrix=RV".to_owned(),
            "output_status_matrix=RSTATUS".to_owned(),
            "output_fit_matrix=RFIT".to_owned(),
            "output_dof_matrix=RDOF".to_owned(),
            "output_fe_matrix=RFE".to_owned(),
            "output_cluster_matrix=RCLUSTER".to_owned(),
            "output_subset_matrix=RSUBSET".to_owned(),
            "output_separation_matrix=RSEP".to_owned(),
            "output_inference_matrix=RINF".to_owned(),
            "output_drop_counts_matrix=RDROP".to_owned(),
            "output_separation_sequence_matrix=RSEQ".to_owned(),
            "output_certificate_matrix=RCERT".to_owned(),
            "output_solver_matrix=RSOLVER".to_owned(),
            "output_runtime_matrix=RRUNTIME".to_owned(),
            "output_receipt_matrix=RREC".to_owned(),
        ];
        (
            args,
            FakeSpi {
                local_bundle: None,
                macro_reads: 0,
                nobs: 10,
                nvars: 10,
                numeric,
                matrices: std::mem::take(&mut matrices),
                reads: 0,
                // Keep the post-snapshot RUNTIME/COMPLETE tail allocation-free
                // in the mock just as it is through the production SPI bridge.
                stores: Vec::with_capacity(1_024),
                store_calls: 0,
                fail_store_at: None,
                interrupt_at: None,
                interrupt_checks: 0,
                interrupt_after_display: false,
                interrupt_when_marker_exists: None,
                displays: Vec::new(),
            },
        )
    }

    fn categorical_absorb_cluster_weight_fixture() -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = fixture();
        let x = SourceId::new(1).expect("x source");
        let h = SourceId::new(2).expect("h source");
        let g = SourceId::new(3).expect("g source");
        let cluster_only = SourceId::new(4).expect("cluster-only source");
        let specification = CompiledModelSpecification {
            model: CompiledTermGraph {
                sources: vec![
                    SourceEntry {
                        id: x,
                        canonical_name: "x".to_owned(),
                    },
                    SourceEntry {
                        id: h,
                        canonical_name: "h".to_owned(),
                    },
                    SourceEntry {
                        id: g,
                        canonical_name: "g".to_owned(),
                    },
                    SourceEntry {
                        id: cluster_only,
                        canonical_name: "cluster_only".to_owned(),
                    },
                ],
                terms: vec![
                    continuous_test_term(1, x),
                    categorical_test_term(2, h, 0.0, true),
                    categorical_test_term(3, h, 1.0, false),
                ],
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorb term"),
                intercept: true,
                categorical_sources: vec![g],
                slope_sources: Vec::new(),
                label: "g".to_owned(),
                saved_name: "fe_g".to_owned(),
            }],
        };
        let wire = ModelWireV2::from_specification(&specification).expect("categorical PPML wire");
        let absorb_component_count = wire.absorb_components.rows.len();
        for (name, matrix) in [
            ("S", wire.sources.rows),
            ("T", wire.terms.rows),
            ("C", wire.components.rows),
            ("A", wire.absorb_terms.rows),
            ("AC", wire.absorb_components.rows),
        ] {
            spi.matrices.insert(name.to_owned(), matrix);
        }
        spi.matrices.insert("CT".to_owned(), vec![vec![1.0, 1.0]]);
        spi.matrices
            .insert("CC".to_owned(), vec![vec![1.0, 1.0, 4.0]]);
        spi.matrices.insert(
            "BIND".to_owned(),
            vec![
                vec![1.0, 1.0, 0.0],
                vec![2.0, 2.0, 0.0],
                vec![3.0, 3.0, 1.0],
                vec![4.0, 3.0, 2.0],
                vec![5.0, 3.0, 3.0],
                vec![6.0, 5.0, 0.0],
                vec![15.0, 3.0, 4.0],
            ],
        );
        spi.matrices.insert("RB".to_owned(), zero_matrix(1, 4));
        spi.matrices.insert("RV".to_owned(), zero_matrix(4, 4));
        spi.matrices.insert(
            "RSTATUS".to_owned(),
            zero_matrix(4, COEFFICIENT_STATUS_COLUMNS),
        );
        spi.matrices
            .insert("RFE".to_owned(), zero_matrix(1, FE_COLUMNS));
        spi.matrices
            .insert("RCLUSTER".to_owned(), zero_matrix(1, CLUSTER_COLUMNS));
        spi.matrices
            .insert("RSUBSET".to_owned(), zero_matrix(1, SUBSET_COLUMNS));

        spi.nobs = 32;
        spi.nvars = 15;
        spi.numeric.clear();
        for row in 0..32 {
            let observation = row + 1;
            let group = row / 4;
            let within = row % 4;
            let x_value = [-1.5_f64, -0.5, 0.5, 1.5][within];
            let h_value = [0.0_f64, 1.0, 0.0, 1.0][within];
            let group_effect = (group as f64 - 3.5) * 0.04;
            let outcome = (0.3 + group_effect + 0.12 * x_value + 0.2 * h_value).exp();
            let frequency_weight = if row % 3 == 0 { 2.0 } else { 1.0 };
            for (variable, value) in [
                (1, 1.0),
                (2, outcome),
                (3, x_value),
                (4, h_value),
                (5, group as f64 + 1.0),
                (6, frequency_weight),
                (15, group as f64 + 1.0),
            ] {
                spi.numeric.insert((variable, observation), (value, false));
            }
            for variable in 7..=14 {
                spi.numeric.insert((variable, observation), (-999.0, false));
            }
        }

        for (key, replacement) in [
            ("source_count", "source_count=4".to_owned()),
            (
                "source_names",
                format!("source_names={}", registry(&wire.source_names)),
            ),
            ("term_count", "term_count=3".to_owned()),
            (
                "canonical_tokens",
                format!("canonical_tokens={}", registry(&wire.canonical_tokens)),
            ),
            ("component_count", "component_count=3".to_owned()),
            (
                "raw_component_operators",
                format!(
                    "raw_component_operators={}",
                    registry(&wire.raw_component_operators)
                ),
            ),
            (
                "raw_time_series_operators",
                format!(
                    "raw_time_series_operators={}",
                    registry(&wire.raw_time_series_operators)
                ),
            ),
            ("absorb_term_count", "absorb_term_count=1".to_owned()),
            (
                "absorb_component_count",
                format!("absorb_component_count={absorb_component_count}"),
            ),
            (
                "absorb_labels",
                format!("absorb_labels={}", registry(&wire.absorb_labels)),
            ),
            (
                "absorb_saved_names",
                format!("absorb_saved_names={}", registry(&wire.absorb_saved_names)),
            ),
            ("cluster_term_count", "cluster_term_count=1".to_owned()),
            (
                "cluster_component_count",
                "cluster_component_count=1".to_owned(),
            ),
            ("cluster_labels", "cluster_labels=1:g".to_owned()),
            ("binding_count", "binding_count=7".to_owned()),
            ("vce", "vce=cluster".to_owned()),
            ("output_sample", "output_sample=7".to_owned()),
            ("output_residual", "output_residual=8".to_owned()),
            ("output_eta", "output_eta=9".to_owned()),
            ("output_mean", "output_mean=10".to_owned()),
            ("output_drop_reason", "output_drop_reason=11".to_owned()),
            ("output_xb", "output_xb=12".to_owned()),
            ("output_absorbed", "output_absorbed=13".to_owned()),
            ("output_saved_count", "output_saved_count=1".to_owned()),
            (
                "output_saved_channels",
                "output_saved_channels=2:14".to_owned(),
            ),
        ] {
            replace_argument(&mut args, key, replacement);
        }
        (args, spi)
    }

    #[cfg(feature = "ffi")]
    fn allocation_failure_phase_fixture(phase: PpmlPhase) -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = categorical_absorb_cluster_weight_fixture();
        replace_argument(&mut args, "threads", "threads=1".to_owned());
        replace_argument(&mut args, "deterministic", "deterministic=1".to_owned());
        let (argument, methods) = match phase {
            PpmlPhase::SeparationFe => (
                "separation=fe",
                SeparationMethods {
                    fixed_effects: true,
                    simplex: false,
                    iterated_rectifier: false,
                    mu: false,
                },
            ),
            PpmlPhase::SeparationSimplex => (
                "separation=simplex,ir",
                SeparationMethods {
                    fixed_effects: false,
                    simplex: true,
                    iterated_rectifier: true,
                    mu: false,
                },
            ),
            PpmlPhase::SeparationIr => (
                "separation=ir",
                SeparationMethods {
                    fixed_effects: false,
                    simplex: false,
                    iterated_rectifier: true,
                    mu: false,
                },
            ),
            PpmlPhase::SeparationMu => (
                "separation=mu",
                SeparationMethods {
                    fixed_effects: false,
                    simplex: false,
                    iterated_rectifier: false,
                    mu: true,
                },
            ),
            _ => (
                "separation=none",
                SeparationMethods {
                    fixed_effects: false,
                    simplex: false,
                    iterated_rectifier: false,
                    mu: false,
                },
            ),
        };
        replace_argument(&mut args, "separation", argument.to_owned());
        spi.matrices.insert(
            "RSEP".to_owned(),
            zero_matrix(separation_method_capacity(methods), SEPARATION_COLUMNS),
        );

        if matches!(
            phase,
            PpmlPhase::SeparationSimplex | PpmlPhase::SeparationIr | PpmlPhase::SeparationMu
        ) {
            // Retain a positive outcome in every absorbed group while giving
            // each numerical detector a genuine Poisson boundary to inspect.
            for observation in 1..=spi.nobs {
                if observation % 4 == 1 {
                    spi.numeric.insert((2, observation), (0.0, false));
                }
            }
        }
        (args, spi)
    }

    #[cfg(feature = "ffi")]
    fn allocation_audit_n100k_no_fe_k0_fixture() -> (Vec<String>, FakeSpi) {
        const OBSERVATIONS: usize = 100_000;
        let (mut args, mut spi) = fixture();
        let wire = ModelWireV2::from_graph(&CompiledTermGraph {
            sources: Vec::new(),
            terms: Vec::new(),
        })
        .expect("empty structural-constant wire");
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
            "BIND".to_owned(),
            vec![vec![1.0, 1.0, 0.0], vec![2.0, 2.0, 0.0]],
        );
        spi.matrices.insert("RB".to_owned(), zero_matrix(1, 1));
        spi.matrices.insert("RV".to_owned(), zero_matrix(1, 1));
        spi.matrices.insert(
            "RSTATUS".to_owned(),
            zero_matrix(1, COEFFICIENT_STATUS_COLUMNS),
        );
        spi.matrices.insert(
            "RSEP".to_owned(),
            zero_matrix(
                separation_method_capacity(SeparationMethods {
                    fixed_effects: false,
                    simplex: false,
                    iterated_rectifier: false,
                    mu: false,
                }),
                SEPARATION_COLUMNS,
            ),
        );
        spi.nobs = OBSERVATIONS;
        spi.nvars = 3;
        spi.numeric.clear();
        for observation in 1..=OBSERVATIONS {
            let outcome = 1.0 + (observation % 5) as f64;
            spi.numeric.insert((1, observation), (1.0, false));
            spi.numeric.insert((2, observation), (outcome, false));
            spi.numeric.insert((3, observation), (-999.0, false));
        }
        // The production SPI stores do not allocate Rust-side event records.
        // Reserve the fake log before the action baseline so the qualification
        // window measures the estimator rather than test harness growth.
        spi.stores = Vec::with_capacity(OBSERVATIONS + 1_024);

        for (key, replacement) in [
            ("source_count", "source_count=0".to_owned()),
            ("source_names", "source_names=.".to_owned()),
            ("term_count", "term_count=0".to_owned()),
            ("canonical_tokens", "canonical_tokens=.".to_owned()),
            ("component_count", "component_count=0".to_owned()),
            (
                "raw_component_operators",
                "raw_component_operators=.".to_owned(),
            ),
            (
                "raw_time_series_operators",
                "raw_time_series_operators=.".to_owned(),
            ),
            ("binding_count", "binding_count=2".to_owned()),
            ("residual", "residual=none".to_owned()),
            ("separation", "separation=none".to_owned()),
            ("threads", "threads=1".to_owned()),
            ("deterministic", "deterministic=1".to_owned()),
            ("output_sample", "output_sample=3".to_owned()),
            ("output_residual", "output_residual=0".to_owned()),
            ("output_xb", "output_xb=0".to_owned()),
            ("output_absorbed", "output_absorbed=0".to_owned()),
            ("output_eta", "output_eta=0".to_owned()),
            ("output_mean", "output_mean=0".to_owned()),
            ("output_drop_reason", "output_drop_reason=0".to_owned()),
        ] {
            replace_argument(&mut args, key, replacement);
        }
        args.push("allocation_audit=1".to_owned());
        (args, spi)
    }

    fn continuous_test_term(id: u32, source: SourceId) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(id).expect("term"),
            order: TermOrder::new(id).expect("order"),
            coefficient_slot: CoefficientSlot::new(id).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "c.x".to_owned(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
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

    fn categorical_test_term(id: u32, source: SourceId, level: f64, base: bool) -> ExpandedTerm {
        ExpandedTerm {
            id: TermId::new(id).expect("term"),
            order: TermOrder::new(id).expect("order"),
            coefficient_slot: CoefficientSlot::new(id).expect("slot"),
            raw_kind: RawTermKind::Factor,
            whole_column_omitted: base,
            canonical_token: format!("{level}.h"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
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

    #[test]
    fn ppml_bundle_decoder_accepts_the_closed_83_field_protocol_only() {
        fn encode(fields: &[String]) -> String {
            let mut bundle = format!("{}:", fields.len());
            for field in fields {
                bundle.push_str(&format!("{}:{field}", field.len()));
            }
            bundle
        }

        let fields = (0..MAX_PPML_PROTOCOL_FIELDS)
            .map(|index| format!("field_{index}=value_{index}"))
            .collect::<Vec<_>>();
        assert_eq!(
            decode_ppml_estimate_bundle(&encode(&fields)).expect("83-field PPML bundle"),
            fields,
        );

        let oversized = (0..=MAX_PPML_PROTOCOL_FIELDS)
            .map(|index| format!("field_{index}=value_{index}"))
            .collect::<Vec<_>>();
        let error = decode_ppml_estimate_bundle(&encode(&oversized))
            .expect_err("84-field PPML bundle must fail closed");
        assert_eq!(error.code, ErrorCode::Syntax);
        assert!(error.message.contains("between one and 83"));
    }

    #[test]
    fn ppml_local_bundle_preserves_science_and_rejects_incomplete_input_before_reads() {
        let (args, mut ordinary) = fixture();
        estimate_ppml_test(&args, &mut ordinary).unwrap();
        let (args, mut local) = fixture();
        let mut payload = format!("{}:", args.len());
        for value in &args {
            payload.push_str(&format!("{}:{value}", value.len()));
        }
        let descriptor = [format!("bundle_local={}", payload.len())];
        local.local_bundle = Some(payload.as_bytes().to_vec());
        estimate_ppml_test(&descriptor, &mut local).unwrap();
        assert_eq!((ordinary.macro_reads, local.macro_reads), (0, 1));
        for name in [
            "RB", "RV", "RSTATUS", "RFIT", "RDOF", "RFE", "RCLUSTER", "RSUBSET", "RSEP", "RINF",
            "RDROP", "RSEQ", "RCERT", "RREC",
        ] {
            assert_eq!(ordinary.matrices[name], local.matrices[name], "{name}");
        }
        assert_eq!(ordinary.reads, local.reads);
        assert_eq!(ordinary.stores.last(), local.stores.last());
        assert_eq!(ordinary.store_calls, local.store_calls);
        let (_, mut truncated) = fixture();
        truncated.local_bundle = Some(payload.as_bytes()[..payload.len() - 1].to_vec());
        assert!(estimate_ppml_test(&descriptor, &mut truncated).is_err());
        assert_eq!(
            (
                truncated.macro_reads,
                truncated.reads,
                truncated.store_calls
            ),
            (1, 0, 0)
        );
        assert_eq!(truncated.receipt_complete(), 0.0);
    }

    #[test]
    fn hidden_ppml_action_commits_a_distinct_receipt_last() {
        let (args, mut spi) = fixture();
        estimate_ppml_test(&args, &mut spi).expect("hidden PPML estimate");
        assert_eq!(spi.receipt_complete(), 1.0);
        assert_eq!(spi.matrices["RREC"][0][0], fereg_core::PPML_RESULT_MAGIC);
        assert_eq!(spi.matrices["RREC"][0][1], 1.0);
        assert_eq!(spi.matrices["RREC"][0][2], 2.0);
        assert_eq!(spi.matrices["RREC"][0][4], 2.0);
        assert_eq!(spi.matrices["RREC"][0][6], 10.0);
        assert_eq!(spi.matrices["RREC"][0][8], 10.0);
        assert_eq!(spi.matrices["RREC"][0][19], INFERENCE_ROWS as f64);
        assert_eq!(spi.matrices["RREC"][0][20], DROP_COUNT_ROWS as f64);
        assert_eq!(spi.matrices["RREC"][0][21], SEPARATION_SEQUENCE_ROWS as f64);
        assert!(spi.matrices["RREC"][0][22] >= 1.0);
        for limb in 24..32 {
            let value = spi.matrices["RREC"][0][limb];
            assert_eq!(value.fract(), 0.0);
            assert!((0.0..=f64::from(u32::MAX)).contains(&value));
        }
        assert_eq!(spi.matrices["RINF"][0][0], 2.0);
        assert_eq!(spi.matrices["RINF"][1][0], 2.0);
        assert_eq!(spi.matrices["RINF"][15][0], 1.0);
        assert_eq!(spi.matrices["RDROP"][0], vec![0.0, 10.0]);
        for reason in 0..DROP_COUNT_ROWS {
            assert_eq!(spi.matrices["RDROP"][reason][0], reason as f64);
        }
        assert_eq!(spi.matrices["RSEQ"][0][0], 1.0);
        assert_eq!(spi.matrices["RSEQ"][0][1], 1.0);
        assert_eq!(spi.matrices["RSEQ"][4][1], 0.0);
        assert_eq!(spi.matrices["RRUNTIME"][40][0], 0.0);
        assert_eq!(spi.matrices["RRUNTIME"][76][0], 1.0);
        assert_eq!(spi.matrices["RRUNTIME"][77][0], 1.0);
        let allocator_rows = [
            spi.matrices["RRUNTIME"][79][0],
            spi.matrices["RRUNTIME"][80][0],
            spi.matrices["RRUNTIME"][81][0],
        ];
        if allocator_rows
            .iter()
            .all(|value| crate::ppml_result::is_stata_publishable(*value))
        {
            assert!(allocator_rows[1] >= allocator_rows[0]);
            assert_eq!(allocator_rows[2], allocator_rows[1] - allocator_rows[0]);
        } else {
            assert!(allocator_rows
                .iter()
                .all(|value| !crate::ppml_result::is_stata_publishable(*value)));
        }
        assert!(spi.matrices["RRUNTIME"][82..95]
            .iter()
            .all(|row| crate::ppml_result::is_stata_publishable(row[0])));
        assert!(spi.matrices["RRUNTIME"][95..108]
            .iter()
            .all(|row| !crate::ppml_result::is_stata_publishable(row[0])));
        assert_eq!(
            spi.stores.last(),
            Some(&StoreEvent::Matrix(
                "RREC".to_owned(),
                1,
                PPML_RESULT_RECEIPT_COMPLETE_COLUMN,
                1.0,
            )),
        );
        assert_eq!(
            spi.stores.get(spi.stores.len() - 2),
            Some(&StoreEvent::Matrix(
                "RRUNTIME".to_owned(),
                RUNTIME_ROWS,
                1,
                spi.matrices["RRUNTIME"][RUNTIME_ROWS - 1][0],
            )),
        );
        for observation in 1..=10 {
            assert_eq!(spi.output(4, observation), Some(1.0));
            assert!(spi
                .output(5, observation)
                .is_some_and(|value| value.abs() < 1e-8));
            assert!(spi.output(7, observation).is_some_and(|value| value > 0.0));
            assert_eq!(spi.output(8, observation), Some(0.0));
            assert!(spi
                .output(9, observation)
                .zip(spi.output(6, observation))
                .is_some_and(|(xb, eta)| (xb - eta).abs() < 1e-12));
            assert!(spi
                .output(10, observation)
                .is_some_and(|value| value.abs() < 1e-12));
        }
    }

    #[test]
    fn default_request_binds_separation_and_fit_to_one_linear_policy() {
        let (args, mut spi) = fixture();
        let specification = EstimatePpmlSpec::parse(&args).expect("default PPML specification");
        let action = specification
            .decode_and_validate(&mut spi)
            .expect("default PPML request");
        let (absorb, fixed_effect_collinearity_tolerance) = action.config.shared_linear_policy();
        assert_eq!(action.linear.absorb, absorb);
        assert_eq!(
            action.linear.fixed_effect_collinearity_tolerance,
            fixed_effect_collinearity_tolerance,
        );
        assert_eq!(action.separation.methods, SeparationMethods::default());
    }

    #[test]
    fn public_solver_names_decode_to_typed_families() {
        for (name, expected) in [
            ("auto", PpmlSolverFamily::Auto),
            ("map", PpmlSolverFamily::Map),
            ("lsmr", PpmlSolverFamily::Lsmr),
            ("cmg", PpmlSolverFamily::Cmg),
        ] {
            let (mut args, _spi) = fixture();
            replace_argument(&mut args, "solver", format!("solver={name}"));
            let specification = EstimatePpmlSpec::parse(&args).expect("solver specification");
            assert_eq!(
                specification.decode_solver().expect("public solver"),
                expected
            );
        }
    }

    #[test]
    fn unknown_and_retired_solver_spellings_fail_closed() {
        let (mut args, _spi) = fixture();
        replace_argument(&mut args, "solver", "solver=cg".to_owned());
        let error = EstimatePpmlSpec::parse(&args)
            .expect("complete request")
            .decode_solver()
            .expect_err("unknown public solver must fail closed");
        assert_eq!(error.code, ErrorCode::Syntax);

        let (mut args, _spi) = fixture();
        args.push("fe_solver=lsmr".to_owned());
        let error = EstimatePpmlSpec::parse(&args)
            .expect_err("retired private solver key must be rejected");
        assert!(error
            .message
            .contains("unknown estimate-ppml field fe_solver"));
    }

    fn with_unpublished_shadow(
        assertion: impl FnOnce(&EstimatePpmlAction, &mut PpmlShadowResult<'_>, &mut FakeSpi),
    ) {
        let (args, mut spi) = fixture();
        let specification = EstimatePpmlSpec::parse(&args).expect("PPML fixture specification");
        let action = specification
            .decode_and_validate(&mut spi)
            .expect("PPML fixture action");
        let loaded = load_ols_input(
            &action.request,
            action.candidate_all,
            action.input_complete,
            &mut spi,
        )
        .expect("PPML fixture input");
        let adjustment = load_adjustment(action.adjustment, &loaded.candidate, &mut spi)
            .expect("PPML fixture adjustment");
        let result = fit_prepared_ppml(
            PreparedPpmlRequestV1 {
                ols: fallible_clone_ols_request(&action.request).expect("request copy"),
                clusters: fallible_clone_clusters(&action.clusters).expect("cluster copy"),
                vce: action.vce,
                ppml: action.config,
                linear: action.linear,
                memory: action.memory,
                memory_preflight_probe: None,
                allocation_snapshot_probe: None,
                phase_allocation_probe: None,
                linear_phase_allocation_probe: None,
                action_allocation_snapshot_probe: None,
                action_allocation_start: None,
                action_allocation_previous_baseline_bytes: None,
                separation: action.separation,
                output: PreparedPpmlOutputRequest {
                    residual: action.residual,
                    xb: action.outputs.xb.is_some(),
                    absorbed: action.outputs.absorbed.is_some(),
                    eta: action.outputs.eta.is_some(),
                    mean: action.outputs.mean.is_some(),
                    requested_observation_columns: action.requested_observation_columns,
                    publication_matrix_cells: action.publication_matrix_cells,
                },
                solver: action.solver,
                inject_fe_recovery_allocation_failure: false,
            },
            LoadedPpmlInput {
                ols: loaded,
                adjustment,
                adjustment_kind: action.adjustment.kind(),
            },
        )
        .expect("PPML fixture fit");
        let mut shadow = build_certified_ppml_shadow(
            &result,
            CertifiedPpmlShadowContext {
                residual: action.residual,
                xb_requested: action.outputs.xb.is_some(),
                separation_methods: action.separation.methods,
                drop_singletons: action.separation.drop_singletons,
                elapsed_seconds: 0.0,
            },
        )
        .expect("PPML fixture shadow");
        assertion(&action, &mut shadow, &mut spi);
    }

    #[test]
    fn bounded_evidence_and_receipt_tampering_fail_before_any_store() {
        with_unpublished_shadow(|action, shadow, spi| {
            validate_shadow(action, shadow, spi).expect("untampered shadow");

            let inference = shadow.inference[0];
            shadow.inference[0] = 3.0;
            assert_eq!(
                validate_shadow(action, shadow, spi)
                    .expect_err("inference tamper")
                    .code,
                ErrorCode::Internal
            );
            shadow.inference[0] = inference;

            shadow.drop_counts[1] += 1.0;
            assert_eq!(
                validate_shadow(action, shadow, spi)
                    .expect_err("drop-count tamper")
                    .code,
                ErrorCode::Internal
            );
            shadow.drop_counts[1] -= 1.0;

            let row = SEPARATION_SEQUENCE_COLUMNS;
            for column in 0..SEPARATION_SEQUENCE_COLUMNS {
                shadow.separation_sequence.swap(column, row + column);
            }
            assert_eq!(
                validate_shadow(action, shadow, spi)
                    .expect_err("sequence order tamper")
                    .code,
                ErrorCode::Internal
            );
            for column in 0..SEPARATION_SEQUENCE_COLUMNS {
                shadow.separation_sequence.swap(column, row + column);
            }

            shadow.receipt[22] += 1.0;
            assert_eq!(
                validate_shadow(action, shadow, spi)
                    .expect_err("execution-count receipt tamper")
                    .code,
                ErrorCode::Internal
            );
            shadow.receipt[22] -= 1.0;

            for (limb, label) in [
                (24, "initial generation hash"),
                (27, "final generation hash"),
                (29, "execution trace digest"),
                (31, "generation-hash digest"),
            ] {
                shadow.receipt[limb] += 1.0;
                assert_eq!(
                    validate_shadow(action, shadow, spi).expect_err(label).code,
                    ErrorCode::Internal,
                    "{label}",
                );
                shadow.receipt[limb] -= 1.0;
            }
            validate_shadow(action, shadow, spi).expect("restored shadow");
            assert_eq!(spi.store_calls, 0);
            assert!(spi.stores.is_empty());
            assert_eq!(spi.receipt_complete(), 0.0);
        });
    }

    #[cfg(feature = "ffi")]
    #[test]
    fn no_route_rejects_before_canonical_numeric_allocation_and_stores_nothing() {
        const CHILD_ENV: &str = "FEREG_PPML_EARLY_PREFLIGHT_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let status = std::process::Command::new(
                std::env::current_exe().expect("current plugin test executable"),
            )
            .env(CHILD_ENV, "1")
            .env("FEREG_MEMORY_LIMIT_BYTES", "1")
            .arg(
                "ppml_estimate::tests::no_route_rejects_before_canonical_numeric_allocation_and_stores_nothing",
            )
            .arg("--exact")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .status()
            .expect("launch isolated early-preflight allocator test");
            assert!(status.success(), "isolated early-preflight test failed");
            return;
        }

        {
            let _guard = crate::instrumentation::lock_action_observation();
            let (args, mut spi) = allocation_audit_n100k_no_fe_k0_fixture();
            let specification = EstimatePpmlSpec::parse(&args).expect("N=100,000 specification");
            let action = specification
                .decode_and_validate(&mut spi)
                .expect("N=100,000 decoded action");
            let loaded = load_ols_input(
                &action.request,
                action.candidate_all,
                action.input_complete,
                &mut spi,
            )
            .expect("N=100,000 loaded input");
            let adjustment = load_adjustment(action.adjustment, &loaded.candidate, &mut spi)
                .expect("N=100,000 adjustment");
            let mut memory = action.memory;
            memory.available_bytes = Some(1);
            memory.embedding_resident_bytes = 0;
            let prepared_request = PreparedPpmlRequestV1 {
                ols: fallible_clone_ols_request(&action.request).expect("request copy"),
                clusters: fallible_clone_clusters(&action.clusters).expect("cluster copy"),
                vce: action.vce,
                ppml: action.config,
                linear: action.linear,
                memory,
                memory_preflight_probe: Some(record_early_preflight_allocator),
                allocation_snapshot_probe: None,
                phase_allocation_probe: None,
                linear_phase_allocation_probe: None,
                action_allocation_snapshot_probe: None,
                action_allocation_start: None,
                action_allocation_previous_baseline_bytes: None,
                separation: action.separation,
                output: PreparedPpmlOutputRequest {
                    residual: action.residual,
                    xb: action.outputs.xb.is_some(),
                    absorbed: action.outputs.absorbed.is_some(),
                    eta: action.outputs.eta.is_some(),
                    mean: action.outputs.mean.is_some(),
                    requested_observation_columns: action.requested_observation_columns,
                    publication_matrix_cells: action.publication_matrix_cells,
                },
                solver: action.solver,
                inject_fe_recovery_allocation_failure: false,
            };
            let prepared_input = LoadedPpmlInput {
                ols: loaded,
                adjustment,
                adjustment_kind: action.adjustment.kind(),
            };

            EARLY_PREFLIGHT_CURRENT_BYTES.store(0, Ordering::Relaxed);
            EARLY_PREFLIGHT_PEAK_BYTES.store(0, Ordering::Relaxed);
            EARLY_PREFLIGHT_EVENTS.store(0, Ordering::Relaxed);
            let (start, _) = crate::ffi::begin_allocation_observation();
            let error = fit_prepared_ppml(prepared_request, prepared_input)
                .expect_err("one-byte budget must reject every PPML route");
            assert_eq!(error.code, ErrorCode::Allocation);
            assert_eq!(error.return_code(), 909);
            assert_eq!(
                EARLY_PREFLIGHT_EVENTS.load(Ordering::Relaxed),
                start.events,
                "classification and route decision must allocate no canonical or temporary buffer",
            );
            assert_eq!(
                EARLY_PREFLIGHT_CURRENT_BYTES.load(Ordering::Relaxed),
                start.current_bytes,
                "no memory may remain allocated at the no-route decision",
            );
            assert_eq!(
                EARLY_PREFLIGHT_PEAK_BYTES.load(Ordering::Relaxed),
                start.peak_bytes,
                "no observation-sized canonical allocation may precede rejection",
            );
        }

        // Exercise the complete plugin transaction under the same imposed
        // one-byte limit. The direct core observation above proves timing;
        // this second arm proves that the ordinary error path cannot publish
        // a prefix or completion receipt.
        let (args, mut spi) = allocation_audit_n100k_no_fe_k0_fixture();
        let error = estimate_ppml_test(&args, &mut spi)
            .expect_err("the plugin transaction must propagate no-route rejection");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(error.return_code(), 909);
        assert_eq!(spi.store_calls, 0);
        assert!(spi.stores.is_empty());
        assert_eq!(spi.receipt_complete(), 0.0);
    }

    #[cfg(feature = "ffi")]
    #[test]
    fn allocation_audit_is_opt_in_and_publishes_real_phase_evidence() {
        const CHILD_ENV: &str = "FEREG_PPML_ALLOCATION_AUDIT_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            // The tracking allocator is process-global by design. Run this
            // qualification assertion in a one-test child so allocations by
            // unrelated parallel unit tests cannot be misattributed to a
            // PPML phase and spuriously exceed its preflight model.
            let status = std::process::Command::new(
                std::env::current_exe().expect("current plugin test executable"),
            )
            .env(CHILD_ENV, "1")
            .arg("ppml_estimate::tests::allocation_audit_is_opt_in_and_publishes_real_phase_evidence")
            .arg("--exact")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .status()
            .expect("launch isolated PPML allocation-audit test");
            assert!(status.success(), "isolated PPML allocation audit failed");
            return;
        }
        let (args, mut normal) = fixture();
        estimate_ppml_test(&args, &mut normal).expect("normal PPML estimate");
        let normal_outer = normal.matrices["RRUNTIME"][1][0];
        assert_eq!(normal.matrices["RRUNTIME"][40][0], 0.0);

        let (mut audited_args, mut audited) = fixture();
        audited_args.push("allocation_audit=1".to_owned());
        estimate_ppml_test(&audited_args, &mut audited).expect("audited PPML estimate");
        let runtime = &audited.matrices["RRUNTIME"];
        assert_eq!(runtime[40][0], 1.0);
        assert!(runtime[41][0] >= 16.0);
        assert!(runtime[1][0] > normal_outer);
        assert_eq!(
            runtime[44][0], 0.0,
            "warm iterations must not allocate identity scratch"
        );
        assert_eq!(
            runtime[72][0], 1.0,
            "linear session reuse must be certified"
        );
        assert_eq!(runtime[75][0], 1.0, "real allocator certificate must pass");
        assert!(runtime[41..76]
            .iter()
            .all(|row| crate::ppml_result::is_stata_publishable(row[0])));
        assert!(runtime[95..108]
            .iter()
            .all(|row| crate::ppml_result::is_stata_publishable(row[0])));
        assert!(runtime[80][0] >= runtime[79][0]);
        assert_eq!(runtime[81][0], runtime[80][0] - runtime[79][0]);
        assert_eq!(audited.receipt_complete(), 1.0);
    }

    #[cfg(feature = "ffi")]
    #[test]
    fn allocation_failure_is_atomic_in_every_fit_lifetime_phase() {
        const CHILD_ENV: &str = "FEREG_PPML_PHASE_ALLOCATION_FAILURE_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let status = std::process::Command::new(
                std::env::current_exe().expect("current plugin test executable"),
            )
            .env(CHILD_ENV, "1")
            .arg("ppml_estimate::tests::allocation_failure_is_atomic_in_every_fit_lifetime_phase")
            .arg("--exact")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .status()
            .expect("launch isolated PPML allocation-failure test");
            assert!(
                status.success(),
                "isolated PPML allocation-failure test failed"
            );
            return;
        }

        for phase in fereg_core::ppml::PPML_PHASES {
            let (args, mut spi) = allocation_failure_phase_fixture(phase);
            crate::ffi::inject_ppml_allocation_failure(phase);
            let result = estimate_ppml_test(&args, &mut spi);
            let fired = crate::ffi::ppml_allocation_failure_fired();
            let successful_events = crate::ffi::test_ppml_phase_allocation_events(phase);
            eprintln!(
                "PPML allocation-failure phase {}: fired={fired}, successful_events={successful_events}",
                phase.name(),
            );
            if fired {
                let error = result.expect_err("injected PPML allocation must fail closed");
                assert_eq!(error.code, ErrorCode::Allocation, "phase {}", phase.name());
                assert_eq!(spi.store_calls, 0, "phase {}", phase.name());
                assert!(spi.stores.is_empty(), "phase {}", phase.name());
                assert_eq!(spi.receipt_complete(), 0.0, "phase {}", phase.name());
            } else {
                result.unwrap_or_else(|error| {
                    panic!(
                        "allocation-free phase {} did not complete normally: {error}",
                        phase.name(),
                    )
                });
                assert_eq!(
                    successful_events,
                    0,
                    "phase {} allocated but did not trigger its armed failure",
                    phase.name(),
                );
                assert_eq!(spi.receipt_complete(), 1.0, "phase {}", phase.name());
            }
        }
    }

    #[cfg(feature = "ffi")]
    #[test]
    fn allocation_audit_n100k_no_fe_k0_accounts_embedding_preparation() {
        const CHILD_ENV: &str = "FEREG_PPML_N100K_ALLOCATION_AUDIT_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let status = std::process::Command::new(
                std::env::current_exe().expect("current plugin test executable"),
            )
            .env(CHILD_ENV, "1")
            .arg(
                "ppml_estimate::tests::allocation_audit_n100k_no_fe_k0_accounts_embedding_preparation",
            )
            .arg("--exact")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .status()
            .expect("launch isolated N=100,000 PPML allocation-audit test");
            assert!(status.success(), "isolated N=100,000 audit failed");
            return;
        }

        let (args, mut spi) = allocation_audit_n100k_no_fe_k0_fixture();
        let estimation = estimate_ppml_test(&args, &mut spi);
        let embedding =
            TEST_LAST_EMBEDDING_RESIDENT_BYTES.load(std::sync::atomic::Ordering::Relaxed);
        if let Err(error) = &estimation {
            eprintln!("n100k_no_fe_k0 embedding={embedding} error={error}");
        }
        estimation.expect("N=100,000 structural-constant audit");
        let modeled_preparation = spi.matrices["RRUNTIME"][82][0];
        let observed_preparation = spi.matrices["RRUNTIME"][95][0];
        assert!(embedding > 0);
        assert!(crate::ppml_result::is_stata_publishable(
            modeled_preparation
        ));
        assert!(crate::ppml_result::is_stata_publishable(
            observed_preparation
        ));
        assert!(observed_preparation <= modeled_preparation);
        assert_eq!(spi.receipt_complete(), 1.0);
        assert_eq!(
            spi.stores.last(),
            Some(&StoreEvent::Matrix(
                "RREC".to_owned(),
                1,
                PPML_RESULT_RECEIPT_COMPLETE_COLUMN,
                PPML_RESULT_RECEIPT_COMPLETE_VALUE,
            )),
        );
        eprintln!(
            "n100k_no_fe_k0 embedding={embedding} modeled_preparation={modeled_preparation} observed_preparation={observed_preparation}",
        );
    }

    #[cfg(feature = "ffi")]
    #[test]
    fn allocation_audit_preflights_worker_team_switches_one_four_one() {
        const CHILD_ENV: &str = "FEREG_PPML_THREAD_SWITCH_ALLOCATION_AUDIT_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let status = std::process::Command::new(
                std::env::current_exe().expect("current plugin test executable"),
            )
            .env(CHILD_ENV, "1")
            .arg(
                "ppml_estimate::tests::allocation_audit_preflights_worker_team_switches_one_four_one",
            )
            .arg("--exact")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .status()
            .expect("launch isolated PPML worker-team switch audit");
            assert!(status.success(), "isolated worker-team switch audit failed");
            return;
        }

        for (action_index, requested_threads) in [1_usize, 4, 1].into_iter().enumerate() {
            let (mut args, mut spi) = allocation_audit_n100k_no_fe_k0_fixture();
            replace_argument(&mut args, "threads", format!("threads={requested_threads}"));
            estimate_ppml_test(&args, &mut spi).unwrap_or_else(|error| {
                panic!(
                    "worker-team switch action {action_index} ({requested_threads} threads) failed: {error}",
                )
            });
            let runtime = &spi.matrices["RRUNTIME"];
            assert_eq!(runtime[9][0], requested_threads as f64);
            assert_eq!(runtime[10][0], requested_threads as f64);
            assert_eq!(
                runtime[40][0],
                1.0,
                "action {action_index} allocation-audit rows 41-78: {:?}",
                &runtime[40..78],
            );
            for phase in 0..fereg_core::ppml::PPML_PHASE_COUNT {
                let modeled = runtime[82 + phase][0];
                let observed = runtime[95 + phase][0];
                assert!(crate::ppml_result::is_stata_publishable(modeled));
                assert!(crate::ppml_result::is_stata_publishable(observed));
                assert!(
                    observed <= modeled,
                    "action {action_index} phase {phase}: observed={observed}, modeled={modeled}",
                );
            }
            let start = runtime[17][0];
            let current = runtime[18][0];
            let peak = runtime[19][0];
            let peak_delta = runtime[20][0];
            let growth = runtime[21][0];
            let ratio = runtime[22][0];
            let modeled_selected = runtime[13][0];
            for value in [start, current, peak, peak_delta, growth, ratio] {
                assert!(crate::ppml_result::is_stata_publishable(value));
            }
            assert!(peak >= start.max(current));
            assert_eq!(peak_delta, peak - start);
            assert_eq!(growth, current - start);
            assert!(peak_delta <= modeled_selected);
            assert!(ratio <= 1.0);
            assert_eq!(runtime[79][0] + runtime[81][0], runtime[80][0]);
            assert_eq!(spi.receipt_complete(), 1.0);
        }
    }

    #[test]
    fn hidden_ppml_action_carries_factor_absorb_cluster_weight_and_saved_fe() {
        let (args, mut spi) = categorical_absorb_cluster_weight_fixture();
        estimate_ppml_test(&args, &mut spi).expect("categorical hidden PPML estimate");
        assert_eq!(spi.receipt_complete(), 1.0);
        assert_eq!(spi.matrices["RREC"][0][3], 3.0);
        assert_eq!(spi.matrices["RREC"][0][4], 4.0);
        assert_eq!(spi.matrices["RREC"][0][9], 1.0);
        assert_eq!(spi.matrices["RREC"][0][10], 1.0);
        assert_eq!(spi.matrices["RREC"][0][11], 1.0);
        assert_eq!(spi.matrices["RREC"][0][15], 0.0);
        assert_eq!(spi.matrices["RREC"][0][17], 1.0);
        assert_eq!(spi.matrices["RSTATUS"][0][1], 1.0);
        assert_eq!(spi.matrices["RSTATUS"][1][1], 2.0);
        assert_eq!(spi.matrices["RSTATUS"][2][1], 1.0);
        assert_eq!(spi.matrices["RSTATUS"][3][1], 7.0);
        assert_eq!(&spi.matrices["RFE"][0][..3], &[1.0, 1.0, 3.0]);
        assert_eq!(&spi.matrices["RCLUSTER"][0], &[1.0, 1.0, 8.0, 0.0]);
        assert_eq!(&spi.matrices["RSUBSET"][0], &[1.0, 8.0, 1.0, 1.0]);
        assert_eq!(spi.matrices["RFIT"][20][0], 1.0);
        assert_eq!(spi.matrices["RFIT"][21][0], 32.0);
        assert_eq!(spi.matrices["RFIT"][22][0], 32.0);
        assert_eq!(spi.matrices["RFIT"][23][0], 1.0);
        for observation in 1..=32 {
            assert_eq!(spi.output(7, observation), Some(1.0));
            assert_eq!(spi.output(11, observation), Some(0.0));
            let eta = spi.output(9, observation).expect("eta");
            let xb = spi.output(12, observation).expect("xb");
            let absorbed = spi.output(13, observation).expect("absorbed");
            let saved = spi.output(14, observation).expect("saved FE");
            assert!((eta - xb - absorbed).abs() < 1e-10);
            assert!((saved - absorbed).abs() < 1e-10);
        }
        assert_eq!(
            spi.stores.last(),
            Some(&StoreEvent::Matrix(
                "RREC".to_owned(),
                1,
                PPML_RESULT_RECEIPT_COMPLETE_COLUMN,
                1.0,
            )),
        );
    }

    #[test]
    fn active_factor_level_that_vanishes_on_rebase_is_syntax_omitted() {
        let (mut args, mut spi) = categorical_absorb_cluster_weight_fixture();
        // The original wire declares h==1 as an active factor slot.  Leave
        // that level on exactly one candidate observation, then make its
        // outcome missing.  The final prepared sample contains only h==0,
        // so core must preserve the original slot as a dynamic
        // SyntaxOmitted entry rather than changing the coefficient axis.
        for observation in 1..=spi.nobs {
            spi.numeric.insert((4, observation), (0.0, false));
        }
        spi.numeric.insert((4, 1), (1.0, false));
        spi.numeric.insert((2, 1), (0.0, true));
        replace_argument(&mut args, "input_complete", "input_complete=0".to_owned());

        estimate_ppml_test(&args, &mut spi).expect("rebased categorical hidden PPML estimate");
        assert_eq!(spi.receipt_complete(), 1.0);
        assert_eq!(spi.matrices["RSTATUS"][1][1], 2.0);
        assert_eq!(spi.matrices["RSTATUS"][2], [3.0, 3.0, 0.0, 1.0]);
        assert_eq!(spi.matrices["RREC"][0][8], 31.0);
    }

    #[test]
    fn saved_fe_recovery_uses_the_accepted_hessian_for_nonexact_counts() {
        let (args, mut spi) = categorical_absorb_cluster_weight_fixture();
        // Unlike the exact exponential fixture, these positive integer counts
        // do not lie exactly on the requested log-linear surface.  The final
        // strict update therefore moves eta/mu and exercises the requirement
        // that saved-FE recovery, inference bread, and the published state all
        // use the same accepted q*mu Hessian frame.
        for observation in 1..=spi.nobs {
            let count = 1.0 + ((observation * 7 + observation / 4) % 5) as f64;
            spi.numeric.insert((2, observation), (count, false));
        }

        estimate_ppml_test(&args, &mut spi).expect("nonexact-count saved-FE estimate");
        assert_eq!(spi.receipt_complete(), 1.0);
        assert_eq!(spi.matrices["RREC"][0][17], 1.0);
        for observation in 1..=spi.nobs {
            let absorbed = spi.output(13, observation).expect("absorbed predictor");
            let saved = spi.output(14, observation).expect("saved fixed effect");
            assert!((saved - absorbed).abs() < 1e-9);
        }
    }

    #[test]
    fn malformed_wire_and_output_conflicts_fail_before_observation_reads() {
        for (key, value, expected) in [
            ("request_major", "99", ErrorCode::ProtocolMismatch),
            ("result_minor", "99", ErrorCode::ProtocolMismatch),
            ("output_sample", "2", ErrorCode::InvalidArgument),
            ("residual", "none", ErrorCode::Syntax),
            ("residual", "bogus", ErrorCode::Syntax),
            ("solver", "cg", ErrorCode::Syntax),
        ] {
            let (mut args, mut spi) = fixture();
            replace_argument(&mut args, key, format!("{key}={value}"));
            let error = estimate_ppml_test(&args, &mut spi).expect_err("must reject");
            assert_eq!(error.code, expected, "{key}");
            assert_eq!(spi.reads, 0, "{key}");
            assert_eq!(spi.store_calls, 0, "{key}");
            assert_eq!(spi.receipt_complete(), 0.0, "{key}");
        }
    }

    #[test]
    fn finite_valid_unrepresentable_result_fails_r430_atomically() {
        let (args, mut spi) = fixture();
        // Each input is finite, nonnegative, and below Stata's missing-value
        // threshold, but their fitted aggregate cannot be represented on the
        // result wire.  Numerical publication failure must never be relabeled
        // malformed data or leak a partial transaction.
        for observation in 1..=spi.nobs {
            spi.numeric.insert((2, observation), (5.5e307, false));
        }
        let error = estimate_ppml_test(&args, &mut spi)
            .expect_err("unrepresentable finite PPML result must fail");
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert_eq!(spi.store_calls, 0);
        assert_eq!(spi.receipt_complete(), 0.0);
    }

    #[test]
    fn stale_receipt_interrupt_and_panic_leave_complete_zero() {
        let (args, mut stale) = fixture();
        stale.matrices.get_mut("RREC").expect("receipt")[0][0] = 1.0;
        assert!(estimate_ppml_test(&args, &mut stale).is_err());
        assert_eq!(stale.reads, 0);
        assert_eq!(stale.store_calls, 0);
        assert_eq!(stale.receipt_complete(), 0.0);

        let (args, mut interrupted) = fixture();
        interrupted.interrupt_at = Some(1);
        let error = estimate_ppml_test(&args, &mut interrupted).expect_err("interrupted");
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(interrupted.store_calls, 0);
        assert_eq!(interrupted.receipt_complete(), 0.0);

        let (mut args, mut panicked) = fixture();
        args.push("inject=panic".to_owned());
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _ = estimate_ppml_test(&args, &mut panicked);
        }));
        assert!(outcome.is_err());
        assert_eq!(panicked.reads, 0);
        assert_eq!(panicked.store_calls, 0);
        assert_eq!(panicked.receipt_complete(), 0.0);
    }

    #[test]
    fn native_break_marker_is_emitted_after_core_entry_and_interrupts_before_stores() {
        let (mut args, mut spi) = fixture();
        args.push("inject=breaknative".to_owned());
        let marker_token = format!(
            "{:032x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        );
        let marker_path = native_break_marker_path(&marker_token);
        args.push(format!("break_marker_token={marker_token}"));
        spi.interrupt_when_marker_exists = Some(marker_path.clone());

        let error = estimate_ppml_test(&args, &mut spi)
            .expect_err("native Break injection must stop inside the core");
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(spi.displays, [NATIVE_BREAK_MARKER]);
        assert_eq!(
            std::fs::read_to_string(&marker_path).expect("native marker file"),
            NATIVE_BREAK_MARKER
        );
        std::fs::remove_file(&marker_path).expect("remove native marker file");
        assert!(
            spi.reads > 0,
            "the marker must follow complete native input loading"
        );
        assert_eq!(spi.store_calls, 0);
        assert!(spi.stores.is_empty());
        assert_eq!(spi.receipt_complete(), 0.0);
    }

    #[test]
    fn native_break_rejects_missing_or_path_like_marker_tokens_before_reads() {
        let (mut missing_args, mut missing_spi) = fixture();
        missing_args.push("inject=breaknative".to_owned());
        let missing = estimate_ppml_test(&missing_args, &mut missing_spi)
            .expect_err("breaknative without a token must fail closed");
        assert_eq!(missing.code, ErrorCode::Syntax);
        assert_eq!(missing_spi.reads, 0);

        let (mut path_args, mut path_spi) = fixture();
        path_args.push("inject=breaknative".to_owned());
        path_args.push("break_marker_token=/tmp/arbitrary.marker".to_owned());
        let path = estimate_ppml_test(&path_args, &mut path_spi)
            .expect_err("a path must never cross the hidden token wire");
        assert_eq!(path.code, ErrorCode::Syntax);
        assert_eq!(path_spi.reads, 0);
    }

    #[test]
    fn licensed_allocation_hook_reaches_fallible_fe_recovery_and_is_action_local() {
        let (mut args, mut injected) = categorical_absorb_cluster_weight_fixture();
        args.push("inject=allocation".to_owned());
        let error = estimate_ppml_test(&args, &mut injected)
            .expect_err("FE-recovery reservation injection must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(error.return_code(), 909);
        assert!(error.message.contains("complete FE recovery mask"));
        assert_eq!(injected.store_calls, 0);
        assert!(injected.stores.is_empty());
        assert_eq!(injected.receipt_complete(), 0.0);

        let (args, mut following) = categorical_absorb_cluster_weight_fixture();
        estimate_ppml_test(&args, &mut following)
            .expect("allocation injection must not leak into the next action");
        assert_eq!(following.receipt_complete(), 1.0);
    }

    #[test]
    fn every_ppml_store_failure_leaves_receipt_incomplete() {
        let (args, mut successful) = fixture();
        estimate_ppml_test(&args, &mut successful).expect("reference stores");
        let store_count = successful.store_calls;
        assert!(store_count > PPML_RESULT_RECEIPT_COLUMNS);
        for failing_ordinal in 1..=store_count {
            let (args, mut spi) = fixture();
            spi.fail_store_at = Some(failing_ordinal);
            assert!(estimate_ppml_test(&args, &mut spi).is_err());
            assert_eq!(spi.receipt_complete(), 0.0, "failure {failing_ordinal}");
        }
    }

    fn zero_matrix(rows: usize, columns: usize) -> Vec<Vec<f64>> {
        vec![vec![0.0; columns]; rows]
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

    fn replace_argument(args: &mut [String], key: &str, replacement: String) {
        let prefix = format!("{key}=");
        args.iter_mut()
            .find(|argument| argument.starts_with(&prefix))
            .unwrap_or_else(|| panic!("missing fixture argument {key}"))
            .clone_from(&replacement);
    }
}
