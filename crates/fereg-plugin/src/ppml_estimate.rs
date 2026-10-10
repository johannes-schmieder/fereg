use crate::model_metadata::{read_cluster_terms, ClusterTables};
use crate::model_metadata::{reserved_filled, syntax};
use crate::publication::store_matrix;
use crate::transport::{
    admit_estimate_transport, decode_output_position_registry, fallible_string,
    read_local_estimate_bundle, reject_unknown_fields, PPML_PROTOCOL,
};
#[cfg(test)]
use fereg_core::metadata::ModelWireV2;
#[path = "ppml_bias.rs"]
pub(crate) mod bias;
use crate::memory_report::{Baseline, Report as MemoryReport, ROWS as MEMORY_ROWS};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

#[cfg(test)]
use crate::estimate::load_ols_input;
use crate::estimate::{
    imposed_memory_limit_bytes, loading_memory, parse_bool, parse_nonnegative_u32,
    parse_positive_f64, parse_positive_u32, parse_positive_usize, parse_u16, parse_usize,
    read_bindings, read_numeric_columns_in_chunks, required,
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
use fereg_core::error::{
    allocation, invalid_argument as invalid, unsupported, ErrorCode, FeregError, Result,
};
use fereg_core::metadata::{
    AbsorbedTerm, CompiledTermGraph, ExpandedTerm, MetadataFlag, SourceEntry, SourceId,
    TermComponent,
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
/// Fields only the PPML estimation protocol accepts.
const PPML_FIELDS: &[&str] = &[
    "request_major",
    "request_minor",
    "adjustment",
    "adjustment_position",
    "residual",
    "allocation_audit",
    "separation",
    "separation_tolerance",
    "separation_max_iterations",
    "simplex_max_iterations",
    "simplex_max_tableau_cells",
    "output_eta",
    "output_mean",
    "bias_method",
    "bias_unit",
    "bias_probes",
    "bias_seed",
    "bias_stream",
    "output_bias_count",
    "output_bias_channels",
    "output_bias_matrix",
    "output_bias_diagnostics",
    "output_bias_support",
    "output_separation_matrix",
    "output_inference_matrix",
    "output_drop_counts_matrix",
    "output_separation_sequence_matrix",
];

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
struct ResultMatrices<T = String> {
    bias: Option<T>,
    bias_diagnostics: Option<T>,
    bias_support: Option<T>,
    b: T,
    covariance: T,
    coefficient_status: T,
    fit: T,
    dof: T,
    fixed_effects: T,
    clusters: T,
    subsets: T,
    separation: T,
    inference: T,
    drop_counts: T,
    separation_sequence: T,
    certificates: T,
    solver: T,
    runtime: T,
    memory_report: T,
    receipt: T,
}

impl<T> ResultMatrices<T> {
    fn names(&self) -> impl Iterator<Item = &T> {
        [
            &self.b,
            &self.covariance,
            &self.coefficient_status,
            &self.fit,
            &self.dof,
            &self.fixed_effects,
            &self.clusters,
            &self.subsets,
            &self.separation,
            &self.inference,
            &self.drop_counts,
            &self.separation_sequence,
            &self.certificates,
            &self.solver,
            &self.runtime,
            &self.memory_report,
            &self.receipt,
        ]
        .into_iter()
        .chain(self.bias.iter())
        .chain(self.bias_diagnostics.iter())
        .chain(self.bias_support.iter())
    }
}
type PreparedResultMatrices = ResultMatrices<PreparedMatrixStoreName>;

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
    bias_channels: Vec<Option<u32>>,
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
            .chain(self.bias_channels.iter().copied())
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
    bias: Option<fereg_core::ppml::PpmlBiasRequest>,
    bias_unit: Option<u32>,
    memory_baseline: Baseline,
    transport_parent_bytes: u128,
    request: OlsRequestV1,
    clusters: Vec<OlsClusterTermV1>,
    vce: PpmlVceRequest,
    candidate_all: bool,
    input_complete: bool,
    adjustment: AdjustmentBinding,
    residual: Option<PpmlResidualKind>,
    config: PpmlConfig,
    linear: PreparedOlsFitConfig,
    expected_effective_threads: usize,
    memory: PpmlMemoryConfig,
    metadata_peak_bytes: u128,
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
    bias: Option<fereg_core::ppml::PpmlBiasRequest>,
    bias_unit: Option<u32>,
    memory_baseline: Baseline,
    transport_parent_bytes: u128,
    transport_peak_bytes: u128,
    model: crate::model_metadata::ModelMetadata,
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

pub fn estimate_ppml(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {
    #[cfg(test)]
    let _action_observation_guard = crate::instrumentation::lock_action_observation();
    let started = Instant::now();
    let resources_start = crate::instrumentation::begin_ppml_observation();
    let (action_probe, action_start, action_previous_baseline) =
        ppml_action_allocation_observation(resources_start);
    let local_bundle = read_local_estimate_bundle(args, spi)?.map(|value| [value]);
    let mut specification = EstimatePpmlSpec::parse(
        local_bundle
            .as_ref()
            .map_or(args, |values| values.as_slice()),
    )?;
    // The expanded local bundle dies here; only the original ABI argv
    // remains live through numerical work and publication.
    specification.transport_parent_bytes = crate::metadata_memory::argument_parent_bytes(args)?;
    drop(local_bundle);
    let action = specification.decode_and_validate(spi)?;
    if action.injection == PpmlDeveloperInjection::Panic {
        panic!("injected hidden PPML transaction panic");
    }
    let mut input_memory = loading_memory(
        &action.request,
        spi.nobs()?,
        usize::from(action.adjustment.position().is_some())
            + usize::from(action.bias_unit.is_some()),
        action.metadata_capacity_bytes() + action.transport_parent_bytes,
        action.memory.usable_bytes(),
    )?;
    let loaded = crate::estimate::load_ols_input_with_action_team(
        &action.request,
        action.candidate_all,
        action.input_complete,
        &mut input_memory,
        spi,
        action.linear.execution.threads,
    )?;
    let adjustment = load_adjustment(action.adjustment, &loaded.candidate, spi)?;
    let bias_units = load_optional_numeric(action.bias_unit, &loaded.candidate, spi)?;
    spi.check_interrupted()?;
    let phase_probe = ppml_phase_allocator_probe(action.allocation_audit);
    let mut prepared_request = prepared_ppml_request(&action, phase_probe)?;
    let prepared_input = LoadedPpmlInput {
        bias_units,
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
    let transport_parent_bytes = action.transport_parent_bytes;
    let result = crate::ppml_execution::run(
        fereg_core::ThreadTeam::new(action.linear.execution.threads),
        move |poll| {
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
                    prepared_request.memory.embedding_resident_bytes =
                        u128::from(embedding_resident_bytes)
                            .checked_add(transport_parent_bytes)
                            .ok_or_else(|| allocation("PPML embedding parent size overflows"))?;
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
            fit_prepared_ppml_interruptible(prepared_request, prepared_input, poll)
        },
        &mut relay_ppml_interrupts(spi, action.injection, &action.break_marker_token),
    )?;
    let publication_extension_started = Instant::now();
    let mut publication_phase = PublicationPhaseScope::begin(phase_probe);
    let publication_memory = action.publication_memory(&result)?;
    publication_memory.admit(result.memory().plan.usable_bytes)?;
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
    publication_memory.validate_capacity(&shadow)?;
    let memory_report = ppml_memory_report(
        &action,
        &result,
        &shadow,
        input_memory.peak(),
        &publication_memory,
        resources_start,
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
    store_runtime_and_complete(&action, &shadow, &memory_report, spi)
}

// Relay core safe points to Stata's calling thread; the developer native-Break
// injection arms its PTY marker at the first IRLS iteration.
fn relay_ppml_interrupts<'a>(
    spi: &'a mut dyn SpiApi,
    injection: PpmlDeveloperInjection,
    break_marker_token: &'a str,
) -> impl FnMut(PpmlInterruptPoint) -> Result<()> + 'a {
    let mut native_break_armed = false;
    move |point: PpmlInterruptPoint| {
        // The scoped executor relays safe-point requests here. This closure
        // keeps the non-Send/non-Sync SPI session on Stata's calling thread.
        spi.check_interrupted()?;
        if injection == PpmlDeveloperInjection::BreakNative
            && !native_break_armed
            && point == PpmlInterruptPoint::IrlsIteration
        {
            native_break_armed = true;
            spi.display(crate::developer_injection::PPML_NATIVE_BREAK_MARKER)?;
            // Arm the PTY controller only after display returns: Linux Stata
            // can classify a Ctrl-C inside SF_display as an SPI display error.
            crate::developer_injection::create_native_break_marker(
                "feppml",
                crate::developer_injection::PPML_NATIVE_BREAK_MARKER,
                break_marker_token,
            )?;
            // Synchronize the interactive fixture after actual core entry.
            // The controller sends Ctrl-C only after observing the marker;
            // remain on Stata's caller thread until its poll reports Break.
            loop {
                spi.check_interrupted()?;
                std::thread::yield_now();
            }
        }
        Ok(())
    }
}

fn prepared_ppml_request(
    action: &EstimatePpmlAction,
    phase_probe: Option<PpmlPhaseAllocatorProbe>,
) -> Result<PreparedPpmlRequestV1> {
    Ok(PreparedPpmlRequestV1 {
        bias: action.bias,
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
    })
}

// Compose the action ledger through publication and the Stata-side forecast.
fn ppml_memory_report(
    action: &EstimatePpmlAction,
    result: &fereg_core::ppml::CertifiedPpmlResult,
    shadow: &PpmlShadowResult<'_>,
    input_peak: u128,
    publication_memory: &crate::ppml_result::PpmlPublicationMemory,
    resources_start: crate::instrumentation::ResourceSnapshot,
) -> Result<MemoryReport> {
    let expected_solver = match result.memory().expected_solver_workspace_peak {
        Some(peak) => peak.expected_heap_bytes().map(u128::from),
        None => Some(result.memory().plan.solver_workspace_bytes),
    };
    // The expected channel never re-enters admission or telemetry validation.
    let expected_plan = expected_solver.map(|solver_workspace_bytes| {
        let mut plan = result.memory().plan;
        plan.solver_workspace_bytes = solver_workspace_bytes;
        plan
    });
    let mut memory = ppml_action_memory_composition(
        result.memory().plan,
        expected_plan,
        Some(input_peak),
        Some(action.metadata_peak_bytes),
    )?;
    let publication = crate::action_memory::Frame {
        parents: publication_memory.parent_bytes,
        workspace: publication_memory.shadow_bytes,
    };
    memory.record(
        crate::action_memory::Stage::Publication,
        publication,
        Some(publication),
        crate::action_memory::Gaps::WORKER_RUNTIME,
    )?;
    if std::env::var_os("FEREG_ACTION_MEMORY_AUDIT").is_some() {
        let resources = crate::instrumentation::snapshot();
        crate::action_memory::emit(
            &memory.ledger,
            resources
                .allocation_peak_bytes
                .saturating_sub(resources_start.allocation_current_bytes),
        )?;
    }
    let forecast = memory
        .expected_peak
        .map(|native_peak| {
            Ok::<_, FeregError>(crate::memory_forecast::Forecast {
                native_peak,
                stata_growth: crate::memory_forecast::Forecast::publication_growth(
                    shadow.drop_reasons.len(),
                    action.publication_matrix_cells as u128,
                    crate::memory_forecast::has_factor_stripes(&action.request.model),
                    action.memory_baseline.before_allocated,
                )?,
            })
        })
        .transpose()?;
    if let Some(forecast) = forecast {
        forecast.trace(
            action.memory_baseline,
            crate::instrumentation::native_live_before(),
        )?;
    }
    MemoryReport::estimated(
        action.memory_baseline,
        crate::instrumentation::native_live_before(),
        memory.ledger.summary(),
        result.memory().plan.safety_headroom_bytes,
        forecast,
    )
}

impl EstimatePpmlAction {
    fn metadata_capacity_bytes(&self) -> u128 {
        use std::mem::size_of;
        self.request.owned_buffer_bytes()
            + self.clusters.capacity() as u128 * size_of::<OlsClusterTermV1>() as u128
            + self
                .clusters
                .iter()
                .map(|cluster| {
                    cluster.label.capacity() as u128
                        + cluster.sources.capacity() as u128 * size_of::<SourceId>() as u128
                })
                .sum::<u128>()
            + (self.outputs.saved_channels.capacity() + self.outputs.bias_channels.capacity())
                as u128
                * size_of::<Option<u32>>() as u128
            + self.break_marker_token.capacity() as u128
            + self
                .matrices
                .names()
                .map(|name| name.as_c_str().to_bytes_with_nul().len() as u128)
                .sum::<u128>()
    }

    fn publication_memory(
        &self,
        result: &fereg_core::ppml::CertifiedPpmlResult,
    ) -> Result<crate::ppml_result::PpmlPublicationMemory> {
        crate::ppml_result::PpmlPublicationMemory::estimate(
            result.memory().plan.embedding_resident_bytes,
            self.metadata_capacity_bytes() + self.transport_parent_bytes,
            result.owned_buffer_bytes(),
            self.publication_matrix_cells,
            result.fixed_effects().len(),
        )
    }
}

impl EstimatePpmlSpec {
    fn cluster_tables(&self) -> ClusterTables<'_> {
        ClusterTables {
            term_matrix: &self.cluster_term_matrix,
            term_count: self.cluster_term_count,
            component_matrix: &self.cluster_component_matrix,
            component_count: self.cluster_component_count,
            labels: &self.cluster_labels,
        }
    }

    fn metadata_shape(&self) -> crate::metadata_memory::MetadataShape {
        let registry_bytes = [
            &self.model.registries.source_names,
            &self.model.registries.canonical_tokens,
            &self.model.registries.raw_component_operators,
            &self.model.registries.raw_time_series_operators,
            &self.model.registries.absorb_labels,
            &self.model.registries.absorb_saved_names,
            &self.cluster_labels,
        ]
        .into_iter()
        .map(crate::metadata_memory::strings_bytes)
        .sum::<u128>();
        let string_bytes = [
            &self.model.source_matrix,
            &self.model.term_matrix,
            &self.model.component_matrix,
            &self.model.absorb_term_matrix,
            &self.model.absorb_component_matrix,
            &self.cluster_term_matrix,
            &self.cluster_component_matrix,
            &self.binding_matrix,
            &self.break_marker_token,
        ]
        .into_iter()
        .chain(self.matrices.names())
        .map(|value| value.capacity() as u128)
        .sum::<u128>();
        crate::metadata_memory::MetadataShape {
            rows: [
                self.model.source_count,
                self.model.term_count,
                self.model.component_count,
                self.model.absorb_term_count,
                self.model.absorb_component_count,
                self.cluster_term_count,
                self.cluster_component_count,
                self.binding_count,
            ],
            transport_parent_bytes: self.transport_parent_bytes,
            specification_bytes: registry_bytes
                + string_bytes
                + self.outputs.bias_channels.capacity() as u128
                    * std::mem::size_of::<Option<u32>>() as u128
                + self.outputs.saved_channels.capacity() as u128
                    * std::mem::size_of::<Option<u32>>() as u128,
        }
    }

    fn parse(args: &[String]) -> Result<Self> {
        let transport_peak_bytes = admit_estimate_transport(args)?;
        let values = PPML_PROTOCOL.key_values(args)?;
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
        let model_counts = crate::model_metadata::ModelMetadata::counts(&values)?;
        let cluster_term_count = parse_usize(&values, "cluster_term_count")?;
        let cluster_component_count = parse_usize(&values, "cluster_component_count")?;
        let bias_request = bias::parse(&values)?;
        let specification = Self {
            bias: bias_request,
            bias_unit: if bias_request.is_some() {
                Some(parse_positive_u32(&values, "bias_unit")?)
            } else {
                None
            },
            memory_baseline: Baseline::parse(&values)?,
            transport_parent_bytes: crate::metadata_memory::argument_parent_bytes(args)?,
            transport_peak_bytes,
            model: crate::model_metadata::ModelMetadata::parse(
                &values,
                model_counts,
                crate::model_metadata::RegistryFraming::Delimited,
            )?,
            cluster_term_matrix: required(&values, "cluster_term_matrix")?.to_owned(),
            cluster_term_count,
            cluster_component_matrix: required(&values, "cluster_component_matrix")?.to_owned(),
            cluster_component_count,
            cluster_labels: crate::model_metadata::decode_registry(
                required(&values, "cluster_labels")?,
                cluster_term_count,
                crate::model_metadata::RegistryFraming::Delimited,
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
                bias_channels: if bias_request.is_some() {
                    let values = decode_output_position_registry(
                        required(&values, "output_bias_channels")?,
                        parse_usize(&values, "output_bias_count")?,
                    )?;
                    if values.iter().any(Option::is_none) {
                        return Err(syntax("partial bias FE outputs are unsupported"));
                    }
                    values
                } else {
                    Vec::new()
                },
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
                bias: values.get("output_bias_matrix").cloned(),
                bias_diagnostics: values.get("output_bias_diagnostics").cloned(),
                bias_support: values.get("output_bias_support").cloned(),
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
                memory_report: required(&values, "output_memory_matrix")?.to_owned(),
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
        reject_unknown_fields(ACTION_ESTIMATE_PPML, &values, PPML_FIELDS)?;
        Ok(specification)
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
        crate::developer_injection::validate_break_marker_token(
            self.injection == PpmlDeveloperInjection::BreakNative,
            &self.break_marker_token,
        )?;

        let memory = PpmlMemoryConfig {
            requested: self.memory,
            available_bytes: imposed_memory_limit_bytes()?,
            poolsize: self.poolsize,
            ..PpmlMemoryConfig::default()
        };
        let metadata_peak_bytes = self
            .metadata_shape()
            .admit(memory.usable_bytes())?
            .max(self.transport_peak_bytes);
        crate::metadata_memory::admit(metadata_peak_bytes, memory.usable_bytes())?;
        let registries = std::mem::take(&mut self.model.registries);
        let wire = self.model.read_wire(
            spi,
            registries,
            [
                "PPML source",
                "PPML term",
                "PPML component",
                "PPML absorbed term",
                "PPML absorbed component",
            ],
        )?;
        let decoded = wire.decode_specification()?;
        drop(wire);
        decoded.model.validate_executable_metadata()?;
        let request = OlsRequestV1 {
            model: decoded.model,
            absorbed_terms: decoded.absorbed_terms,
            bindings: read_bindings(spi, &self.binding_matrix, self.binding_count)?,
            include_intercept: self.include_intercept,
            keep_singletons: self.keep_singletons,
        };
        let clusters = read_cluster_terms(spi, self.cluster_tables(), &request, Some(5))?;
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
        if let Some(position) = self.bias_unit {
            validate_adjustment_binding(AdjustmentBinding::Offset(position), spi)?;
        }
        validate_variable_layout(&request, adjustment, self.bias_unit, &outputs, spi)?;
        if self.bias.is_some() != self.matrices.bias.is_some()
            || self.bias.is_some() != self.matrices.bias_diagnostics.is_some()
            || self.bias.is_some() != self.matrices.bias_support.is_some()
        {
            return Err(syntax("bias request and output matrices disagree"));
        }
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
                &self.model.source_matrix,
                &self.model.term_matrix,
                &self.model.component_matrix,
                &self.model.absorb_term_matrix,
                &self.model.absorb_component_matrix,
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
            bias: self.bias,
            bias_unit: self.bias_unit,
            memory_baseline: self.memory_baseline,
            transport_parent_bytes: self.transport_parent_bytes,
            request,
            clusters,
            vce,
            candidate_all: self.candidate_all,
            input_complete: self.input_complete,
            adjustment,
            residual,
            config,
            expected_effective_threads: linear.execution.threads.resolved(),
            linear,
            memory,
            metadata_peak_bytes,
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

/// Parse the hidden PPML control plane without infallibly duplicating the
/// potentially coefficient-sized registry values. Protocol field count is
/// fixed and bounded; each key and value payload is copied only after a
/// fallible reservation.
fn load_adjustment(
    binding: AdjustmentBinding,
    candidate: &[bool],
    spi: &mut dyn SpiApi,
) -> Result<Option<LoadedNumericColumn>> {
    load_optional_numeric(binding.position(), candidate, spi)
}

fn load_optional_numeric(
    position: Option<u32>,
    candidate: &[bool],
    spi: &mut dyn SpiApi,
) -> Result<Option<LoadedNumericColumn>> {
    let Some(position) = position else {
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

#[cfg(all(test, feature = "ffi"))]
fn ppml_action_memory_ledger(
    plan: fereg_core::ppml::PpmlMemoryPlan,
    input: Option<u128>,
    metadata: Option<u128>,
) -> Result<crate::action_memory::Ledger> {
    Ok(ppml_action_memory_composition(plan, None, input, metadata)?.ledger)
}

fn ppml_action_memory_composition(
    plan: fereg_core::ppml::PpmlMemoryPlan,
    expected: Option<fereg_core::ppml::PpmlMemoryPlan>,
    input: Option<u128>,
    metadata: Option<u128>,
) -> Result<crate::action_memory::Composition> {
    use crate::action_memory::{Composition, Estimator, Frame, Gaps, Stage};
    let mut composition = Composition::new(Estimator::Ppml);
    let embedding = plan.selected_inventory().embedding_resident_bytes;
    for (stage, peak, gaps) in [
        (
            Stage::Metadata,
            metadata.unwrap_or(embedding),
            Gaps::TRANSPORT_COMPILER,
        ),
        (
            Stage::Loading,
            input.unwrap_or(embedding),
            if input.is_some() {
                Gaps::TRANSPORT_COMPILER
            } else {
                Gaps::STRING_LOADING.union(Gaps::TRANSPORT_COMPILER)
            },
        ),
    ] {
        let frame = Frame {
            parents: 0,
            workspace: peak,
        };
        composition.record(stage, frame, Some(frame), gaps)?;
    }
    let generation = Gaps::WORKER_RUNTIME
        .union(Gaps::DEPENDENCY_CONTAINERS)
        .union(Gaps::PPML_GENERATION_PARENT);
    let numerical = generation.union(Gaps::NUMERICAL_FRAMES);
    for (stage, gaps) in [
        (Stage::Preparation, generation.union(Gaps::CONSTRUCTION)),
        (Stage::Separation, generation),
        (Stage::Fit, numerical),
        (Stage::Iteration, numerical),
        (Stage::Strict, numerical),
        (Stage::Inference, numerical),
        (Stage::Recovery, generation.union(Gaps::RECOVERY)),
        (Stage::Publication, generation.union(Gaps::PUBLICATION)),
    ] {
        let admission = ppml_stage_frame(plan, stage)?;
        let expected = expected
            .map(|plan| ppml_stage_frame(plan, stage))
            .transpose()?;
        composition.record(stage, admission, expected, gaps)?;
    }
    Ok(composition)
}

fn ppml_stage_frame(
    plan: fereg_core::ppml::PpmlMemoryPlan,
    stage: crate::action_memory::Stage,
) -> Result<crate::action_memory::Frame> {
    use crate::action_memory::{Frame, Stage};
    let phase = |phase| plan.modeled_phase_peak_bytes(phase);
    let peak = match stage {
        Stage::Preparation => phase(PpmlPhase::Preparation),
        Stage::Separation => phase(PpmlPhase::SeparationFe).max(phase(PpmlPhase::SeparationMu)),
        Stage::Fit => phase(PpmlPhase::FeProjection),
        Stage::Iteration => phase(PpmlPhase::WeightResponseRefresh)
            .max(phase(PpmlPhase::ObjectiveLineSearch))
            .max(phase(PpmlPhase::FeProjection)),
        Stage::Strict => phase(PpmlPhase::StrictFinalSolve),
        Stage::Inference => phase(PpmlPhase::Inference),
        Stage::Recovery => phase(PpmlPhase::FeRecovery),
        Stage::Publication => phase(PpmlPhase::Publication),
        Stage::Metadata | Stage::Loading => {
            return Err(internal("PPML core stage requires a phase"))
        }
    };
    let parents = plan.selected_inventory().embedding_resident_bytes;
    Ok(Frame {
        parents,
        workspace: peak
            .checked_sub(parents)
            .ok_or_else(|| internal("PPML action phase omits its embedding parent"))?,
    })
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
    bias::validate(action, shadow, spi)?;
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
    for name in action.matrices.names() {
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
        // Automatic estimation uses the common structural projection family.
        // A canonical checkpoint records its independent numerical operation;
        // it does not change the selected ordinary solver.
        if selected != f64::from(PpmlSolverFamily::Map.code()) {
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
                || expected_selected == f64::from(PpmlSolverFamily::Cmg.code())
                || (expected_selected == f64::from(PpmlSolverFamily::Map.code())
                    && action.request.absorbed_terms.len() == 3
                    && action
                        .request
                        .absorbed_terms
                        .iter()
                        .all(|term| term.intercept && term.slope_sources.is_empty()))));
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
        || shadow.effective_threads != action.expected_effective_threads
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
        let expected_omitted = if term.whole_column_omitted
            && term
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
            || expected_omitted.is_none() && !matches!(status as u8, 1 | 4 | 5 | 6)
        {
            return Err(internal(
                "certified PPML coefficient status disagrees with the model slot recipe",
            ));
        }
    }
    let constant_slot = action.request.model.terms.len();
    let constant_status = shadow.coefficient_status[constant_slot * COEFFICIENT_STATUS_COLUMNS + 1];
    let valid_constant = if action
        .request
        .absorbed_terms
        .iter()
        .any(|term| term.intercept)
    {
        constant_status == 7.0
    } else {
        // Preserved factor recipes can span the constant without an absorbed
        // intercept. Its generated recipe must survive, but QR may omit it.
        matches!(constant_status as u8, 1 | 6)
    };
    if !valid_constant {
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
            if let Some(bias) = &shadow.bias {
                for (position, values) in action.outputs.bias_channels.iter().zip(bias.saved) {
                    spi.store_numeric(
                        position.ok_or_else(|| internal("missing bias output position"))?,
                        zero_based + 1,
                        values[retained_cursor],
                    )?;
                }
            }
            retained_cursor += 1;
        }
        if let Some(position) = action.outputs.drop_reason {
            spi.store_numeric(position, zero_based + 1, f64::from(reason.code()))?;
        }
    }
    if let Some(bias) = &shadow.bias {
        store_matrix(
            spi,
            action
                .matrices
                .bias
                .as_ref()
                .ok_or_else(|| internal("bias matrix missing"))?,
            bias::CHANNELS,
            shadow.coefficient_count,
            &bias.coefficients,
        )?;
        store_matrix(
            spi,
            action
                .matrices
                .bias_diagnostics
                .as_ref()
                .ok_or_else(|| internal("bias diagnostics missing"))?,
            bias::DIAGNOSTICS,
            1,
            &bias.diagnostics,
        )?;
        store_matrix(
            spi,
            action
                .matrices
                .bias_support
                .as_ref()
                .ok_or_else(|| internal("bias support missing"))?,
            2,
            bias::SUPPORT_COLUMNS,
            &bias.support,
        )?;
    }
    let dimension = shadow.coefficient_count.max(1);
    for (name, rows, columns, values) in [
        (&action.matrices.b, 1, shadow.b.len(), &shadow.b[..]),
        (
            &action.matrices.covariance,
            dimension,
            dimension,
            &shadow.covariance[..],
        ),
        (
            &action.matrices.coefficient_status,
            dimension,
            COEFFICIENT_STATUS_COLUMNS,
            &shadow.coefficient_status[..],
        ),
        (&action.matrices.fit, FIT_ROWS, 1, &shadow.fit[..]),
        (&action.matrices.dof, DOF_ROWS, 1, &shadow.dof[..]),
        (
            &action.matrices.fixed_effects,
            shadow.fixed_effect_rows.max(1),
            FE_COLUMNS,
            &shadow.fixed_effects[..],
        ),
        (
            &action.matrices.clusters,
            shadow.cluster_rows.max(1),
            CLUSTER_COLUMNS,
            &shadow.clusters[..],
        ),
        (
            &action.matrices.subsets,
            shadow.subset_rows.max(1),
            SUBSET_COLUMNS,
            &shadow.subsets[..],
        ),
        (
            &action.matrices.separation,
            shadow.separation_rows.max(1),
            SEPARATION_COLUMNS,
            &shadow.separation[..],
        ),
        (
            &action.matrices.inference,
            INFERENCE_ROWS,
            1,
            &shadow.inference[..],
        ),
        (
            &action.matrices.drop_counts,
            DROP_COUNT_ROWS,
            DROP_COUNT_COLUMNS,
            &shadow.drop_counts[..],
        ),
        (
            &action.matrices.separation_sequence,
            SEPARATION_SEQUENCE_ROWS,
            SEPARATION_SEQUENCE_COLUMNS,
            &shadow.separation_sequence[..],
        ),
        (
            &action.matrices.certificates,
            CERTIFICATE_ROWS,
            1,
            &shadow.certificates[..],
        ),
        (
            &action.matrices.solver,
            SOLVER_ROWS,
            SOLVER_COLUMNS,
            &shadow.solver[..],
        ),
    ] {
        store_matrix(spi, name, rows, columns, values)?;
    }
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
    memory_report: &MemoryReport,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    store_matrix(
        spi,
        &action.matrices.memory_report,
        MEMORY_ROWS,
        2,
        &memory_report.0,
    )?;
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
    matrices.names().try_fold(0_usize, |total, matrix| {
        let cells = matrix
            .rows()
            .checked_mul(matrix.columns())
            .ok_or_else(|| allocation("PPML publication matrix cell count overflows"))?;
        total
            .checked_add(cells)
            .ok_or_else(|| allocation("PPML publication matrix cell count overflows"))
    })
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
    bias_unit: Option<u32>,
    outputs: &ObservationOutputs,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    let nvars = spi.nvars()?;
    let output_count = outputs.positions().count();
    let expected = request
        .bindings
        .len()
        .checked_add(
            usize::from(adjustment.position().is_some()) + usize::from(bias_unit.is_some()),
        )
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
        // Categorical sources may be string-coded; every other input is read
        // as numeric, as the OLS layout requires.
        let numeric = match binding.role {
            OlsInputRole::NumericSource(source) => {
                crate::estimate::source_requires_numeric(request, source)
            }
            _ => true,
        };
        if numeric && spi.variable_is_string(position)? {
            return Err(unsupported(format!(
                "plugin varlist position {position} must be numeric"
            )));
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
    if let Some(position) = bias_unit {
        if request
            .bindings
            .iter()
            .any(|b| b.stata_var_position == position)
            || adjustment.position() == Some(position)
        {
            return Err(invalid("bias unit input aliases another input position"));
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
            || bias_unit == Some(position)
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
    let prepared = PreparedResultMatrices {
        bias: matrices
            .bias
            .as_ref()
            .map(|name| PreparedMatrixStoreName::try_new(name, bias::CHANNELS, dimension))
            .transpose()?,
        bias_diagnostics: matrices
            .bias_diagnostics
            .as_ref()
            .map(|name| PreparedMatrixStoreName::try_new(name, bias::DIAGNOSTICS, 1))
            .transpose()?,
        bias_support: matrices
            .bias_support
            .as_ref()
            .map(|name| PreparedMatrixStoreName::try_new(name, 2, bias::SUPPORT_COLUMNS))
            .transpose()?,
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
        memory_report: PreparedMatrixStoreName::try_new(&matrices.memory_report, MEMORY_ROWS, 2)?,
        runtime: PreparedMatrixStoreName::try_new(&matrices.runtime, RUNTIME_ROWS, 1)?,
        receipt: PreparedMatrixStoreName::try_new(
            &matrices.receipt,
            1,
            PPML_RESULT_RECEIPT_COLUMNS,
        )?,
    };
    let input_names = input_matrices
        .iter()
        .map(|name| name.as_str())
        .collect::<BTreeSet<_>>();
    let mut output_names = BTreeSet::new();
    for matrix in prepared.names() {
        let name = matrix.as_str();
        let (rows, columns) = (matrix.rows(), matrix.columns());
        if name.is_empty() || input_names.contains(name) || !output_names.insert(name) {
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
    Ok(prepared)
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

fn optional_position(values: &BTreeMap<String, String>, key: &str) -> Result<Option<u32>> {
    let position = parse_nonnegative_u32(values, key)?;
    Ok((position != 0).then_some(position))
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

fn protocol(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::ProtocolMismatch, message)
}

fn internal(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}

#[cfg(test)]
mod tests;
