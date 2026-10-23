use crate::input_memory::{InputMemory, StringDictionary};
use crate::memory_report::{Baseline, Report as MemoryReport, ROWS as MEMORY_ROWS};
pub(crate) use crate::model_metadata::{
    exact_usize, parse_bool, parse_nonnegative_u32, parse_positive_f64, parse_positive_u32,
    parse_positive_usize, parse_u16, parse_usize, push_reserved, read_bindings, required,
    reserved_empty, reserved_filled, syntax,
};
use crate::model_metadata::{read_cluster_terms, ClusterTables};
use crate::publication::store_matrix;
use crate::transport::{
    admit_estimate_transport, decode_output_position_registry, read_local_estimate_bundle,
    reject_unknown_fields, OLS_PROTOCOL,
};
#[cfg(test)]
use fereg_core::ols::OlsInputBinding;
use std::collections::BTreeSet;
use std::time::Instant;

use crate::input::{PreparedMatrixStoreName, SpiApi};
use crate::instrumentation::{self, ResourceSnapshot};
use crate::result::{
    build_shadow_result, CertifiedOlsInference, OlsShadowResult, OlsVceKind, CERTIFICATE_ROWS,
    CLUSTER_COLUMNS, COEFFICIENT_STATUS_COLUMNS, DOF_ROWS, FE_COLUMNS, FIT_ROWS, RECEIPT_COLUMNS,
    RECEIPT_COMPLETE_COLUMN, RESULT_MAGIC, RUNTIME_ROWS, SOLVER_COLUMNS, SOLVER_ROWS,
    SUBSET_COLUMNS,
};
use fereg_core::error::{
    allocation, invalid_argument as invalid, invalid_data, unsupported, ErrorCode, FeregError,
    Result,
};
use fereg_core::fe::{AbsorbConfig, FeSolverKind, LsmrConfig, LsmrPreconditioner, RoutingPreset};
use fereg_core::metadata::{ComponentKind, SourceId};
#[cfg(test)]
use fereg_core::ols::{
    fit_prepared_model_unadjusted_with_loaded_moments, prepare_model_owned, OlsClusteredRequestV1,
};
use fereg_core::ols::{
    CertifiedPreparedOlsFit, LoadedNumericColumn, LoadedOlsInput, LoadedOlsRows, OlsClusterTermV1,
    OlsInputRole, OlsRequestV1, PreparedOlsAbsorbedDofPolicy, PreparedOlsClusteredConfig,
    PreparedOlsColumnKind, PreparedOlsExecutionConfig, PreparedOlsExecutionMode,
    PreparedOlsFitConfig, PreparedOlsFitStatus, PreparedOlsLoadHint, PreparedOlsRobustConfig,
    PreparedOlsUnadjustedConfig,
};
use fereg_core::protocol::{
    Handshake, ACTION_PROTOCOL_VERSION, OLS_RESULT_WIRE_VERSION, PLUGIN_VERSION,
};
use fereg_core::{
    estimate_phase2_memory, CategoricalTopologyDofReport, FeSolverMemoryBudget, MapConfig,
    MemoryMode, MemoryPlan, PreparedOlsData, ThreadConfig, ThreadTeam,
};
use rayon::prelude::*;

pub const ACTION_ESTIMATE_OLS: &str = "estimate-ols";
const CONFIRMATION: &str = "fereg-estimation";
/// Fields only the OLS estimation protocol accepts.
const OLS_FIELDS: &[&str] = &[
    "dof",
    "account_cluster_nesting",
    "preconditioner",
    "cmgdegree",
    "cmgsamples",
    "cmgseed",
    "output_xbd",
    "output_marker_b_matrix",
    "output_marker_v_matrix",
];

#[derive(Debug, Clone)]
struct ResultMatrices<T = String> {
    b: T,
    covariance: T,
    coefficient_status: T,
    omission_b_marker: T,
    omission_v_marker: T,
    fit: T,
    dof: T,
    solver: T,
    fixed_effects: T,
    clusters: T,
    subsets: T,
    certificates: T,
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
            &self.omission_b_marker,
            &self.omission_v_marker,
            &self.fit,
            &self.dof,
            &self.solver,
            &self.fixed_effects,
            &self.clusters,
            &self.subsets,
            &self.certificates,
            &self.runtime,
            &self.memory_report,
            &self.receipt,
        ]
        .into_iter()
    }
}
type PreparedResultMatrices = ResultMatrices<PreparedMatrixStoreName>;

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
    loaded_rhs_layout: Option<crate::numeric_pipeline::NumericRhsLayout>,
    prepared_owner_bytes: u128,
    execution_parent_bytes: u128,
    output_parent_bytes: u128,
    metadata_owner_bytes: u128,
    metadata_peak_bytes: u128,
    input_peak_bytes: u128,
    loaded_fit_workspace_bytes: u128,
    publication_memory: crate::ols_publication_memory::OlsPublicationMemory,
    requested_tolerance: f64,
    outputs: ObservationOutputs,
    matrices: PreparedResultMatrices,
    /// Hidden-adapter Break injection token; `None` for every ordinary action.
    native_break_token: Option<String>,
}

#[derive(Debug)]
struct EstimateActionSpec {
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
    vce: OlsVceKind,
    absorbed_dof: PreparedOlsAbsorbedDofPolicy,
    account_cluster_nesting: bool,
    memory: MemoryMode,
    solver: FeSolverKind,
    routing: Option<RoutingPreset>,
    ownership: fereg_core::ols::PreparedOlsOwnership,
    it_coefficients: bool,
    cmg_schur_degree: usize,
    cmg_schur_samples: u32,
    cmg_schur_seed: u64,
    preconditioner: LsmrPreconditioner,
    poolsize: usize,
    threads: usize,
    deterministic: bool,
    tolerance: f64,
    iterate: usize,
    outputs: ObservationOutputs,
    matrices: ResultMatrices,
    native_break_token: Option<String>,
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

pub fn estimate_ols(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {
    #[cfg(test)]
    let _action_observation_guard = instrumentation::lock_action_observation();
    let started = Instant::now();
    let resources_start = instrumentation::begin_observation();
    let local_bundle = read_local_estimate_bundle(args, spi)?.map(|value| [value]);
    let mut specification = EstimateActionSpec::parse(
        local_bundle
            .as_ref()
            .map_or(args, |values| values.as_slice()),
    )?;
    // The expanded local bundle dies here; only the original ABI argv
    // remains live through numerical work and publication.
    specification.transport_parent_bytes = crate::metadata_memory::argument_parent_bytes(args)?;
    drop(local_bundle);
    let mut action = specification.decode_and_validate(spi)?;
    drop(specification);
    let preflight_seconds = started.elapsed().as_secs_f64();
    let mut input_memory = loading_memory(
        &action.request,
        spi.nobs()?,
        0,
        action.metadata_owner_bytes,
        action.memory_plan.usable_bytes,
    )?;
    let input_team = (action.candidate_all && numeric_pipeline_effects_supported(&action.request))
        .then(|| ThreadTeam::new(action.fit.execution.threads));
    let mut loaded_rhs = None;
    let mut load_columns = None;
    let rhs_budget = action.loaded_rhs_budget();
    let loaded = load_ols_input_with_team(
        &action.request,
        action.candidate_all,
        action.input_complete,
        &mut input_memory,
        spi,
        input_team,
        &mut loaded_rhs,
        rhs_budget,
        action.loaded_rhs_layout,
        false,
        Some(crate::numeric_pipeline::PreparedLoadTarget::Owned(
            &mut load_columns,
        )),
    )?;
    action.loaded_fit_workspace_bytes = loaded_rhs
        .as_ref()
        .map_or(0, |rows| rows.capacity() as u128 * 8);
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
    let trace_enabled = !categorical
        && action.vce != OlsVceKind::Clustered
        && fereg_core::diagnostics::phase_timing_enabled();
    let trace_started = Instant::now();
    // Preparation, inference and derived observation outputs (including
    // saved-FE recovery) run on the action team while Stata's thread polls
    // Break; an interruption stops core loops at their next checkpoint.
    let team = ThreadTeam::new(action.fit.execution.threads);
    let mut native_break_token = action.native_break_token.take();
    let builder_break = action
        .fit
        .absorb
        .routing
        .is_some_and(RoutingPreset::breaks_builder);
    let hold_for_break = native_break_token.is_some() && !builder_break;
    let numerical_action = &mut action;
    let (prepared, inference, derived, phase_seconds) = crate::cancellable::run(
        team,
        move || {
            if hold_for_break {
                // Hidden Break injection: wait at a checkpoint for Stata Break.
                loop {
                    fereg_core::interrupt::checkpoint()?;
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
            let (prepared, loaded_rows) = prepare_ols_pipeline(
                loaded,
                loaded_rhs,
                load_columns,
                numerical_action,
                categorical,
            )?;
            numerical_action.admit_prepared_parent(
                prepared.execution_admission_bytes(),
                prepared.owned_buffer_bytes(),
            )?;
            let prepare_seconds = trace_started.elapsed().as_secs_f64();
            let inference_started = Instant::now();
            let inference = infer_ols(&prepared, numerical_action, loaded_rows)?;
            numerical_action.admit_output_parent(
                prepared.execution_admission_bytes(),
                inference.view().owned_buffer_bytes(),
            )?;
            let inference_seconds = inference_started.elapsed().as_secs_f64();
            let outputs_started = Instant::now();
            let derived =
                derive_observation_outputs(&prepared, inference.view(), numerical_action)?;
            let outputs_seconds = outputs_started.elapsed().as_secs_f64();
            Ok((
                prepared,
                inference,
                derived,
                [prepare_seconds, inference_seconds, outputs_seconds],
            ))
        },
        &mut || poll_ols_wait(spi, &mut native_break_token, builder_break),
    )?;
    let view = inference.view();
    complete_ols(
        &action,
        &prepared,
        prepared.cluster_design().map(|_| &prepared),
        view,
        &derived,
        &observation,
        spi,
        trace_enabled.then_some(OlsPhaseTrace {
            label: if action.vce == OlsVceKind::Iid {
                "iid_plugin"
            } else {
                "robust_plugin"
            },
            started: trace_started,
            phase_seconds,
        }),
    )
}

fn poll_ols_wait(
    spi: &mut dyn SpiApi,
    native_break_token: &mut Option<String>,
    builder_break: bool,
) -> Result<()> {
    let ready = !builder_break || fereg_core::fe::routing_builder_break_ready();
    if let Some(token) = ready.then(|| native_break_token.take()).flatten() {
        // Arm the interactive controller only after display returns.
        spi.display(crate::developer_injection::OLS_NATIVE_BREAK_MARKER)?;
        crate::developer_injection::create_native_break_marker(
            "fereg",
            crate::developer_injection::OLS_NATIVE_BREAK_MARKER,
            &token,
        )?;
    }
    spi.check_interrupted_while_waiting()
}

mod ols_pipeline;
use ols_pipeline::{infer_ols, prepare_ols_pipeline};

struct OlsPhaseTrace {
    label: &'static str,
    started: Instant,
    /// Preparation, inference and derived-output seconds.
    phase_seconds: [f64; 3],
}

// One canonical prepared owner feeds the three mathematical covariance variants.
// Borrowed views preserve prepared owners until the validated result is stored.
#[allow(clippy::too_many_arguments)]
fn complete_ols(
    action: &EstimateAction,
    data: &'_ PreparedOlsData,
    clusters: Option<&'_ PreparedOlsData>,
    inference: CertifiedOlsInference<'_>,
    derived: &DerivedObservationOutputs,
    observation: &ActionObservation,
    spi: &mut dyn SpiApi,
    trace: Option<OlsPhaseTrace>,
) -> Result<()> {
    let shadow_started = Instant::now();
    let shadow = build_shadow_result(data, clusters, inference, action.outputs.residual.is_some())?;
    let shadow_seconds = shadow_started.elapsed().as_secs_f64();
    if let Some(trace) = trace {
        let [prepare, inference, outputs] = trace.phase_seconds;
        eprintln!(
            "FEREG_PHASE|{}|prepare={prepare:.9}|inference={inference:.9}|outputs={outputs:.9}|shadow={shadow_seconds:.9}|prestore_total={:.9}",
            trace.label,
            trace.started.elapsed().as_secs_f64()
        );
    }
    precommit_and_store(action, &shadow, derived, observation, spi)
}

// Preparation and raw moments borrow disjoint Rust-owned inputs. The optional
// cache has the same whole-action admission and deferred errors as the IID path.
fn prepare_loaded_ols_with_moments<T: Send>(
    loaded: LoadedOlsInput,
    loaded_rhs: Option<Vec<f64>>,
    action: &mut EstimateAction,
    prepare: impl FnOnce(&OlsRequestV1, LoadedOlsInput, u128, Option<u128>) -> Result<T> + Send,
) -> Result<(T, Option<LoadedOlsRows>)> {
    let started = Instant::now();
    let nrows = loaded.candidate.len();
    let columns = action
        .generated_columns
        .checked_add(1)
        .ok_or_else(|| allocation("loaded OLS moment width overflows"))?;
    let moment_bytes = LoadedOlsRows::moment_workspace_bytes(nrows, columns).filter(|bytes| {
        loaded_rhs.is_some()
            && loaded.analytic_weight.is_none()
            && loaded.frequency_weight.is_none()
            && loaded.probability_weight.is_none()
            && fereg_core::parallel::StagePlan::new(
                nrows as u128 * columns as u128,
                2,
                action.fit.execution.threads.resolved(),
            )
            .lanes
                > 1
            && action.memory_plan.usable_bytes.is_none_or(|usable| {
                action
                    .preparation_memory
                    .preparation_peak_bytes(action.loaded_fit_workspace_bytes + *bytes as u128)
                    .ok()
                    .and_then(|peak| peak.checked_add(action.metadata_owner_bytes))
                    .is_some_and(|peak| peak <= usable)
            })
    });
    // Preserve the existing moment-cache decision before spending headroom on
    // transient membership scratch. Weighted and complex preparations do not
    // reuse this membership route.
    let membership_scratch = if action.memory_plan.selected == MemoryMode::Fast
        && action.fit.execution.threads.resolved() > 1
        && loaded.analytic_weight.is_none()
        && loaded.frequency_weight.is_none()
        && loaded.probability_weight.is_none()
        && !action.request.absorbed_terms.is_empty()
    {
        action.preparation_memory.admit_membership_scratch(
            fereg_core::fe::LevelIndex::parallel_scratch_bytes(nrows),
            action.memory_plan.usable_bytes,
            action.metadata_owner_bytes
                + action.loaded_fit_workspace_bytes
                + moment_bytes.unwrap_or(0) as u128,
        )?
    } else {
        0
    };
    let external = action
        .metadata_owner_bytes
        .checked_add(action.loaded_fit_workspace_bytes)
        .and_then(|bytes| bytes.checked_add(moment_bytes.unwrap_or(0) as u128));
    let release_ceiling = external.and_then(|external| {
        let ceiling = action.preparation_memory.preparation_peak_bytes(0).ok()?;
        Some(
            action
                .memory_plan
                .usable_bytes
                .map_or(ceiling, |limit| ceiling.min(limit.saturating_sub(external))),
        )
    });
    // A candidate-row/full-generated-width upper bound proves the later packed
    // cache fits even after singleton/sample compaction. Large-cache, IID, Low
    // and weighted paths keep their raw moments. Keep the original reservation
    // in both branches: this optimization cannot buy different FE admission.
    let skip_moments = action.fit.ownership.skip_unused_moments
        && unused_packed_moments(action.memory_plan.selected, action.vce, nrows, columns);
    let result = if let Some(bytes) = moment_bytes.filter(|_| !skip_moments) {
        action.loaded_fit_workspace_bytes += bytes as u128;
        let request = &action.request;
        ThreadTeam::new(action.fit.execution.threads).install(|| {
            let (prepared, rows) = rayon::join(
                || prepare(request, loaded, membership_scratch, release_ceiling),
                || {
                    loaded_rhs.map(|rows| {
                        LoadedOlsRows::from(rows).with_unit_moments(nrows, columns, bytes)
                    })
                },
            );
            prepared.map(|prepared| (prepared, rows))
        })?
    } else {
        action.loaded_fit_workspace_bytes += moment_bytes.unwrap_or(0) as u128;
        if skip_moments && fereg_core::diagnostics::phase_timing_enabled() {
            eprintln!(
                "FEREG_OWNERSHIP|unused_moment_bytes={}",
                moment_bytes.unwrap_or(0)
            );
        }
        let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
            prepare(&action.request, loaded, membership_scratch, release_ceiling)
        })?;
        Ok((prepared, loaded_rhs.map(LoadedOlsRows::from)))
    };
    if fereg_core::diagnostics::phase_timing_enabled() {
        eprintln!(
            "FEREG_PHASE|ols_prepare_moments|seconds={:.9}",
            started.elapsed().as_secs_f64()
        );
    }
    result
}

fn unused_packed_moments(mode: MemoryMode, vce: OlsVceKind, rows: usize, columns: usize) -> bool {
    mode == MemoryMode::Fast
        && vce != OlsVceKind::Iid
        && rows
            .checked_mul(columns)
            .and_then(|elements| elements.checked_mul(std::mem::size_of::<f64>()))
            .is_some_and(|bytes| bytes <= 32 * 1024 * 1024)
}

// Loading has finished before this boundary; the closure captures no SPI session.
// Reuse the solve's bounded team for native classification and compaction.
fn prepare_loaded_ols<T: Send>(
    loaded: LoadedOlsInput,
    threads: ThreadConfig,
    prepare: impl FnOnce(LoadedOlsInput) -> Result<T> + Send,
) -> Result<T> {
    let started = Instant::now();
    let prepared = if fereg_core::parallel::StagePlan::new(
        loaded.candidate.len() as u128 * loaded.numeric_sources.len() as u128 * 2,
        loaded.numeric_sources.len(),
        threads.resolved(),
    )
    .lanes
        > 1
    {
        ThreadTeam::new(threads).install(|| prepare(loaded))??
    } else {
        prepare(loaded)?
    };
    if fereg_core::diagnostics::phase_timing_enabled() {
        eprintln!(
            "FEREG_PHASE|ols_prepare|seconds={:.9}",
            started.elapsed().as_secs_f64()
        );
    }
    Ok(prepared)
}

impl EstimateAction {
    /// Admit the optional packed RHS beside the existing preparation peak.
    fn loaded_rhs_budget(&self) -> usize {
        if self.fit.execution.mode != PreparedOlsExecutionMode::Fast {
            return 0;
        }
        self.memory_plan.usable_bytes.map_or(usize::MAX, |usable| {
            usize::try_from(
                usable
                    .saturating_sub(self.metadata_owner_bytes)
                    .saturating_sub(
                        self.preparation_memory
                            .preparation_peak_bytes(0)
                            .unwrap_or(u128::MAX),
                    ),
            )
            .unwrap_or(usize::MAX)
        })
    }

    /// One stage composition supplies conservative coverage and expected heap.
    /// Admission parents are retained even when the expected finite and FE
    /// frames are sequential and therefore have smaller overlapping parents.
    fn memory_composition(
        &self,
        solver: Option<u64>,
        expected_fit: Option<u128>,
    ) -> Result<crate::action_memory::Composition> {
        use crate::action_memory::{Composition, Estimator, Frame, Gaps, Stage};
        let mut composition = Composition::new(Estimator::Ols);
        for (stage, frame, gaps) in [
            (
                Stage::Metadata,
                Frame {
                    parents: 0,
                    workspace: self.metadata_peak_bytes,
                },
                Gaps::TRANSPORT_COMPILER,
            ),
            (
                Stage::Loading,
                Frame {
                    parents: 0,
                    workspace: self.input_peak_bytes,
                },
                Gaps::TRANSPORT_COMPILER.union(Gaps::WORKER_RUNTIME),
            ),
            (
                Stage::Preparation,
                Frame {
                    parents: self.metadata_owner_bytes,
                    workspace: self
                        .preparation_memory
                        .preparation_peak_bytes(self.loaded_fit_workspace_bytes)?,
                },
                Gaps::CONSTRUCTION.union(Gaps::WORKER_RUNTIME),
            ),
        ] {
            composition.record(stage, frame, Some(frame), gaps)?;
        }
        let mut gaps = Gaps::NUMERICAL_FRAMES
            .union(Gaps::RETAINED_FE_PARENT)
            .union(Gaps::WORKER_RUNTIME);
        if !self.request.absorbed_terms.is_empty() {
            gaps = gaps.union(Gaps::DEPENDENCY_CONTAINERS);
        }
        if solver.is_none() {
            gaps = gaps.union(Gaps::SOLVER_ATTEMPT);
        }
        let fit = Frame {
            parents: self.execution_parent_bytes,
            workspace: u128::from(solver.unwrap_or(0)),
        };
        let expected = expected_fit.map(|workspace| Frame {
            parents: 0,
            workspace,
        });
        for stage in [Stage::Fit, Stage::Inference] {
            composition.record(stage, fit, expected, gaps)?;
        }
        let recovery = Frame {
            parents: self.output_parent_bytes,
            workspace: self.publication_memory.derived_peak_bytes,
        };
        composition.record(
            Stage::Recovery,
            recovery,
            Some(recovery),
            if self.outputs.saved_channels.iter().any(Option::is_some) {
                Gaps::RECOVERY
                    .union(Gaps::WORKER_RUNTIME)
                    .union(Gaps::DEPENDENCY_CONTAINERS)
            } else {
                Gaps::WORKER_RUNTIME
            },
        )?;
        let publication = Frame {
            parents: self.output_parent_bytes,
            workspace: self.publication_memory.peak_bytes(),
        };
        composition.record(
            Stage::Publication,
            publication,
            Some(publication),
            Gaps::WORKER_RUNTIME,
        )?;
        Ok(composition)
    }

    #[cfg(all(test, feature = "ffi"))]
    fn memory_ledger(&self, solver: Option<u64>) -> Result<crate::action_memory::Ledger> {
        Ok(self.memory_composition(solver, None)?.ledger)
    }

    fn memory_evidence(
        &self,
        shadow: &OlsShadowResult<'_>,
    ) -> Result<(
        crate::action_memory::Ledger,
        Option<crate::memory_forecast::Forecast>,
    )> {
        use crate::memory_forecast::{ols_projection_parent, sum, Forecast};
        let expected_fit = shadow
            .solver_expected_heap_peak
            .map(|solver| {
                let projection = if shadow.execution.mode == PreparedOlsExecutionMode::Fast {
                    ols_projection_parent(
                        self.prepared_owner_bytes,
                        self.metadata_owner_bytes,
                        shadow.prepared.sample().retained_rows().len(),
                        self.generated_columns,
                        self.fit.execution.poolsize,
                        true,
                    )?
                } else {
                    self.execution_parent_bytes
                };
                let projection = sum(&[projection, u128::from(solver)])?;
                Ok::<_, FeregError>(if shadow.execution.mode == PreparedOlsExecutionMode::Fast {
                    projection.max(sum(&[
                        self.prepared_owner_bytes,
                        self.metadata_owner_bytes,
                        self.preparation_memory.frame_metadata_bytes,
                        self.frame_memory.fast_bytes,
                    ])?)
                } else {
                    projection
                })
            })
            .transpose()?;
        let composition = self.memory_composition(shadow.solver_workspace_peak, expected_fit)?;
        let forecast = composition
            .expected_peak
            .map(|native_peak| {
                let matrix_cells = self.publication_memory.shadow_bytes / 8
                    + (crate::result::RUNTIME_ROWS * 2 + crate::memory_report::CELLS) as u128;
                Ok::<_, FeregError>(Forecast {
                    native_peak,
                    stata_growth: Forecast::publication_growth(
                        shadow.prepared.sample().drop_reasons.len(),
                        matrix_cells,
                        crate::memory_forecast::has_factor_stripes(&self.request.model),
                        self.memory_baseline.before_allocated,
                    )?,
                })
            })
            .transpose()?;
        Ok((composition.ledger, forecast))
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
            + matrices.names().map(|name| name.as_c_str().to_bytes_with_nul().len() as u128).sum::<u128>()
    }

    /// Reconcile the surviving preparation owner before fit/inference allocates
    /// its frames. Public base estimates retain their historical meaning.
    fn admit_prepared_parent(&mut self, owned_bytes: u128, actual_owned_bytes: u128) -> Result<()> {
        let public_base = self.memory_plan.estimate;
        let reconciled = self
            .preparation_memory
            .with_prepared_owner(public_base, owned_bytes)?;
        let frame_parent = owned_bytes
            .checked_add(self.preparation_memory.frame_metadata_bytes)
            .ok_or_else(|| allocation("OLS transient metadata parent overflows"))?;
        let mut execution = self.frame_memory.with_parent(reconciled, frame_parent)?;
        execution.fast_bytes = execution
            .fast_bytes
            .checked_add(self.metadata_owner_bytes)
            .ok_or_else(|| allocation("OLS execution metadata size overflows"))?;
        execution.low_bytes = execution
            .low_bytes
            .checked_add(self.metadata_owner_bytes)
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
        self.fit.absorb.solver_memory_budget =
            plan.usable_bytes.map(|usable_bytes| FeSolverMemoryBudget {
                execution_bytes,
                usable_bytes,
            });
        self.fit.retained_score_capacity_limit = (plan.selected == MemoryMode::Fast
            && self.vce != OlsVceKind::Iid
            && self.fit.ownership.retain_projected_scores)
            .then_some(self.frame_memory.retained_score_capacity_limit);
        self.fit.absorb.retain_coefficients =
            self.outputs.saved_channels.iter().any(Option::is_some)
                && plan.selected == MemoryMode::Fast;
        plan.estimate = public_base;
        self.memory_plan = plan;
        self.prepared_owner_bytes = actual_owned_bytes;
        self.execution_parent_bytes = execution_bytes;
        Ok(())
    }

    fn admit_output_parent(
        &mut self,
        prepared_admission_bytes: u128,
        inference_bytes: u128,
    ) -> Result<()> {
        let owned = prepared_admission_bytes
            .checked_add(inference_bytes)
            .and_then(|bytes| bytes.checked_add(self.metadata_owner_bytes))
            .ok_or_else(|| allocation("OLS output parent size overflows"))?;
        self.output_parent_bytes = owned;
        if let Some(budget) = self.fit.absorb.solver_memory_budget.as_mut() {
            let parent = budget.execution_bytes.max(owned);
            if parent
                .checked_add(self.publication_memory.peak_bytes())
                .is_none_or(|peak| peak > budget.usable_bytes)
            {
                return Err(allocation(
                    "OLS retained inference and publication exceed usable memory",
                ));
            }
            budget.execution_bytes = parent;
        }
        Ok(())
    }
}

/// Validate the explicit CMG-Schur option family as one request contract.
fn parse_cmg_schur_options(
    values: &std::collections::BTreeMap<String, String>,
    solver: FeSolverKind,
) -> Result<(usize, u32, u64)> {
    let cmg_schur_degree = match values.get("cmgdegree") {
        Some(_) if solver != FeSolverKind::CmgSchur => {
            return Err(syntax("cmgdegree() is valid only with solver(cmgschur)"));
        }
        Some(_) => {
            let degree = parse_usize(values, "cmgdegree")?;
            if !(4..=6).contains(&degree) {
                return Err(syntax("cmgdegree() must be 4, 5, or 6"));
            }
            degree
        }
        None => 5,
    };
    for option in ["cmgsamples", "cmgseed"] {
        if values.contains_key(option) && solver != FeSolverKind::CmgSchur {
            return Err(syntax(format!(
                "{option}() is valid only with solver(cmgschur)"
            )));
        }
    }
    let cmg_schur_samples = values
        .get("cmgsamples")
        .map(|_| parse_nonnegative_u32(values, "cmgsamples"))
        .transpose()?
        .unwrap_or(0);
    let cmg_schur_seed = match values.get("cmgseed") {
        Some(_) if cmg_schur_samples == 0 => {
            return Err(syntax("cmgseed() requires positive cmgsamples()"));
        }
        Some(value) => value
            .parse::<u64>()
            .ok()
            .filter(|seed| *seed < (1_u64 << 53))
            .ok_or_else(|| syntax("cmgseed() must be an integer in 0..9007199254740991"))?,
        None => 0,
    };
    Ok((cmg_schur_degree, cmg_schur_samples, cmg_schur_seed))
}

impl EstimateActionSpec {
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
                + self.outputs.saved_channels.capacity() as u128
                    * std::mem::size_of::<Option<u32>>() as u128,
        }
    }

    fn parse(args: &[String]) -> Result<Self> {
        let transport_peak_bytes = admit_estimate_transport(args)?;
        let values = OLS_PROTOCOL.key_values(args)?;
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
        let model_counts = crate::model_metadata::ModelMetadata::counts(&values)?;
        let cluster_term_count = parse_usize(&values, "cluster_term_count")?;
        let cluster_component_count = parse_usize(&values, "cluster_component_count")?;
        let output_residual = parse_nonnegative_u32(&values, "output_residual")?;
        let output_drop_reason = parse_nonnegative_u32(&values, "output_drop_reason")?;
        let output_xb = parse_nonnegative_u32(&values, "output_xb")?;
        let output_xbd = parse_nonnegative_u32(&values, "output_xbd")?;
        let output_absorbed = parse_nonnegative_u32(&values, "output_absorbed")?;
        let output_saved_count = parse_usize(&values, "output_saved_count")?;
        let solver = parse_solver(required(&values, "solver")?)?;
        let injection = crate::developer_injection::parse_ols(
            values.get("inject").map_or("", String::as_str),
            solver,
            values.get("break_marker_token").map_or("", String::as_str),
        )?;
        let threads = parse_usize(&values, "threads")?;
        let (cmg_schur_degree, cmg_schur_samples, cmg_schur_seed) =
            parse_cmg_schur_options(&values, solver)?;
        let specification = Self {
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
            vce: parse_vce(required(&values, "vce")?)?,
            absorbed_dof: parse_dof(required(&values, "dof")?)?,
            account_cluster_nesting: parse_bool(&values, "account_cluster_nesting")?,
            memory: MemoryMode::parse(required(&values, "memory")?)?,
            solver,
            routing: injection.routing,
            ownership: injection.ownership,
            it_coefficients: injection.it_coefficients,
            cmg_schur_degree,
            cmg_schur_samples,
            cmg_schur_seed,
            preconditioner: parse_preconditioner(required(&values, "preconditioner")?)?,
            poolsize: parse_positive_usize(&values, "poolsize")?,
            threads,
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
            native_break_token: injection.native_break_token,
        };
        reject_unknown_fields(ACTION_ESTIMATE_OLS, &values, OLS_FIELDS)?;
        Ok(specification)
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
        let metadata_peak_bytes = self
            .metadata_shape()
            .admit(usable_bytes)?
            .max(self.transport_peak_bytes);
        crate::metadata_memory::admit(metadata_peak_bytes, usable_bytes)?;
        let wire = self.model.read_wire(
            spi,
            self.model.registries.clone(),
            [
                "source",
                "term",
                "component",
                "absorbed term",
                "absorbed component",
            ],
        )?;
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
        if self.solver == FeSolverKind::CmgSchur
            && (request.absorbed_terms.len() != 2
                || request.absorbed_terms.iter().any(|term| {
                    !term.intercept
                        || term.categorical_sources.is_empty()
                        || !term.slope_sources.is_empty()
                }))
        {
            return Err(unsupported(
                "solver(cmgschur) requires exactly two categorical intercept fixed effects",
            ));
        }
        let clusters = read_cluster_terms(spi, self.cluster_tables(), &request, None)?;
        validate_vce(self.vce, &clusters, self.account_cluster_nesting)?;
        let categorical = request.absorbed_terms.iter().any(|term| {
            term.categorical_sources.len() > 1
                || !term.slope_sources.is_empty()
                || !term.saved_name.is_empty()
        });
        let prepared_model =
            fereg_core::ols::compile_prepared_model(&request, &clusters, categorical)?;
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
        let memory_plan = publication_memory.plan(self.memory, memory_estimate, available_bytes)?;
        let preparation_memory = crate::ols_preparation_memory::OlsPreparationMemory::estimate(
            spi.nobs()?,
            loaded_numeric_columns,
            request.bindings.iter().any(|binding| {
                matches!(
                    binding.role,
                    OlsInputRole::AnalyticWeight
                        | OlsInputRole::FrequencyWeight
                        | OlsInputRole::ProbabilityWeight
                )
            }),
            extended_fe_rows,
            clusters.len(),
        )?;
        let preparation_memory =
            preparation_memory.with_construction(spi.nobs()?, &request, &clusters)?;
        preparation_memory.admit_numeric_arrays(memory_plan.usable_bytes)?;
        let frame_memory = crate::ols_frame_memory::OlsFrameMemory::estimate(
            spi.nobs()?,
            prepared_model.columns.len(),
            prepared_model.coefficients.len(),
            clusters.len(),
            self.poolsize,
            self.vce != OlsVceKind::Iid,
        )?;
        let default_fit = PreparedOlsFitConfig::default();
        let thread_config = ThreadConfig::from_requested(self.threads, self.deterministic)?;
        let fit = PreparedOlsFitConfig {
            ownership: self.ownership,
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
                routing: self.routing,
                it_coefficients: self.it_coefficients,
                cmg_schur_degree: self.cmg_schur_degree,
                cmg_schur_samples: self.cmg_schur_samples,
                cmg_schur_seed: self.cmg_schur_seed,
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
            loaded_rhs_layout: numeric_loaded_rhs_layout(&prepared_model),
            prepared_owner_bytes: 0,
            execution_parent_bytes: 0,
            output_parent_bytes: 0,
            metadata_owner_bytes: 0,
            metadata_peak_bytes,
            input_peak_bytes: 0,
            loaded_fit_workspace_bytes: 0,
            publication_memory,
            requested_tolerance: self.tolerance,
            outputs: self.outputs.clone(),
            matrices,
            native_break_token: self.native_break_token.clone(),
        };
        action.metadata_owner_bytes = action
            .metadata_capacity_bytes()
            .checked_add(action.transport_parent_bytes)
            .ok_or_else(|| allocation("metadata parent size overflows"))?;
        action.preparation_memory.admit_numeric_arrays_with_parent(
            action.memory_plan.usable_bytes,
            action.metadata_owner_bytes,
        )?;
        action.admit_prepared_parent(0, 0)?;
        Ok(action)
    }
}

// This is a metadata transport capacity, independent of N and finite rank.
// Refuse impossible descriptors before reserving or accessing the caller macro.
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

#[cfg(test)]
pub(crate) fn load_ols_input(
    request: &OlsRequestV1,
    candidate_all: bool,
    input_complete: bool,
    spi: &mut dyn SpiApi,
) -> Result<LoadedOlsInput> {
    let mut memory = loading_memory(request, spi.nobs()?, 0, 0, None)?;
    load_ols_input_with_team(
        request,
        candidate_all,
        input_complete,
        &mut memory,
        spi,
        None,
        &mut None,
        0,
        None,
        false,
        None,
    )
}

pub(crate) fn load_preparation_input(
    request: &OlsRequestV1,
    spi: &mut dyn SpiApi,
) -> Result<LoadedOlsInput> {
    let mut memory = loading_memory(request, spi.nobs()?, 0, 0, None)?;
    load_ols_input_with_team(
        request,
        false,
        false,
        &mut memory,
        spi,
        None,
        &mut None,
        0,
        None,
        true,
        None,
    )
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
    let weights = request
        .bindings
        .iter()
        .filter(|binding| {
            matches!(
                binding.role,
                OlsInputRole::AnalyticWeight
                    | OlsInputRole::FrequencyWeight
                    | OlsInputRole::ProbabilityWeight
            )
        })
        .count();
    let columns = sources
        .checked_add(weights)
        .and_then(|v| v.checked_add(extra_columns))
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| allocation("input column count overflows"))?;
    let row_bytes = add(mul(columns, 9)?, 1)?;
    let arrays = (rows as u128)
        .checked_mul(row_bytes)
        .ok_or_else(|| allocation("input array bytes overflow"))?;
    // The loader and final source descriptors can coexist. Position lists,
    // whole-column and chunk slices, plus the SPI's positions/pointers are
    // column-sized, with at most one bulk call live on the Stata thread.
    let sources_bytes = mul(
        sources,
        size_of::<(SourceId, OlsSourceLoader)>()
            + size_of::<(SourceId, LoadedNumericColumn)>()
            + size_of::<u32>(),
    )?;
    let transport_bytes = mul(
        columns,
        2 * size_of::<u32>()
            + size_of::<*mut f64>()
            + 2 * size_of::<&mut [f64]>()
            + 2 * size_of::<&mut [bool]>(),
    )?;
    InputMemory::new(
        usable_bytes,
        add(
            parent_bytes,
            add(arrays, add(sources_bytes, transport_bytes)?)?,
        )?,
    )
}

/// Load with the action's worker team when every candidate is retained and
/// the effects support the tiled numeric pipeline, as the OLS action does.
/// Loaded values are identical; only the reading schedule changes.
pub(crate) fn load_ols_input_with_action_team(
    request: &OlsRequestV1,
    candidate_all: bool,
    input_complete: bool,
    memory: &mut InputMemory,
    spi: &mut dyn SpiApi,
    threads: fereg_core::ThreadConfig,
    load_hint: Option<&mut Option<PreparedOlsLoadHint>>,
) -> Result<LoadedOlsInput> {
    let team = (candidate_all && numeric_pipeline_effects_supported(request))
        .then(|| ThreadTeam::new(threads));
    load_ols_input_with_team(
        request,
        candidate_all,
        input_complete,
        memory,
        spi,
        team,
        &mut None,
        0,
        None,
        false,
        load_hint.map(crate::numeric_pipeline::PreparedLoadTarget::Borrowed),
    )
}

fn numeric_pipeline_effects_supported(request: &OlsRequestV1) -> bool {
    !request.absorbed_terms.is_empty()
        && request.absorbed_terms.iter().all(|term| {
            term.categorical_sources.len() == 1
                && (term.slope_sources.is_empty() || request.absorbed_terms.len() == 1)
                && (term.intercept || (1..=3).contains(&term.slope_sources.len()))
        })
}

// Use the already validated finite model, including its cluster-source graph.
// Loading only materializes this recipe; the final fit checks every row/value.
fn numeric_loaded_rhs_layout(
    model: &fereg_core::ols::PreparedOlsModel,
) -> Option<crate::numeric_pipeline::NumericRhsLayout> {
    let append_intercept = matches!(
        model.columns.last().map(|c| &c.kind),
        Some(PreparedOlsColumnKind::Intercept)
    );
    let numeric = model
        .columns
        .len()
        .saturating_sub(usize::from(append_intercept));
    model.columns[..numeric].iter().enumerate().all(|(index, column)|
        matches!(column.kind, PreparedOlsColumnKind::Continuous { prepared_source_index, .. } if index == prepared_source_index))
        .then_some(crate::numeric_pipeline::NumericRhsLayout { width: model.columns.len() + 1, append_intercept })
}

// Preserve the explicit SPI admission and ownership inputs of the legacy loader.
#[allow(clippy::too_many_arguments)]
fn load_ols_input_with_team(
    request: &OlsRequestV1,
    candidate_all: bool,
    input_complete: bool,
    memory: &mut InputMemory,
    spi: &mut dyn SpiApi,
    team: Option<ThreadTeam>,
    rhs: &mut Option<Vec<f64>>,
    rhs_budget: usize,
    compiled_rhs_layout: Option<crate::numeric_pipeline::NumericRhsLayout>,
    candidate_first: bool,
    mut load_hint: Option<crate::numeric_pipeline::PreparedLoadTarget<'_>>,
) -> Result<LoadedOlsInput> {
    let n = spi.nobs()?;
    let mut candidate = if candidate_all {
        fereg_core::data::UniformValues::Repeated {
            len: n,
            value: true,
        }
    } else {
        reserved_filled(n, false, "OLS candidate marker")?.into()
    };
    let mut candidate_position = if candidate_all {
        None
    } else {
        Some(role_position(request, InputRoleTag::Candidate)?)
    };
    if candidate_first {
        if let Some(position) = candidate_position.take() {
            // Developer diagnostics require the complete marker to be valid
            // before any outcome/source observation is read.
            for (row, retained) in candidate
                .explicit_mut()
                .expect("marker reads own explicit storage")
                .iter_mut()
                .enumerate()
            {
                spi.check_interrupted()?;
                *retained = read_candidate_marker(spi, position, row + 1)?;
            }
        }
    }
    let has_weight = request.bindings.iter().any(|binding| {
        matches!(
            binding.role,
            OlsInputRole::AnalyticWeight
                | OlsInputRole::FrequencyWeight
                | OlsInputRole::ProbabilityWeight
        )
    });
    let numeric_width = request.model.sources.len() + 1 + usize::from(has_weight);
    let defer_columns = candidate_all
        && (1..=64).contains(&numeric_width)
        && team.is_some_and(|t| {
            fereg_core::parallel::StagePlan::new(n as u128 * numeric_width as u128, 2, t.threads())
                .lanes
                > 1
        })
        && spi.supports_numeric_rows();
    let mut outcome = loaded_column_reservation(n, "OLS outcome", defer_columns)?;
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
            OlsSourceLoader::Numeric(loaded_column_reservation(
                n,
                "OLS numeric source",
                defer_columns,
            )?)
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
        .map(|_| loaded_column_reservation(n, "analytic weight", defer_columns))
        .transpose()?;
    let mut frequency_weight = frequency_position
        .map(|_| loaded_column_reservation(n, "frequency weight", defer_columns))
        .transpose()?;
    let mut probability_weight = probability_position
        .map(|_| loaded_column_reservation(n, "probability weight", defer_columns))
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
    let mut rhs_layout = None;
    if candidate_all && all_sources_numeric {
        if let Some(team) = team {
            let width = numeric_width;
            if spi.supports_numeric_rows()
                && fereg_core::parallel::StagePlan::new(
                    n as u128 * width as u128,
                    2,
                    team.threads(),
                )
                .lanes
                    > 1
                && (1..=64).contains(&width)
                && numeric_pipeline_effects_supported(request)
            {
                if let Some(layout) = compiled_rhs_layout.filter(|layout| {
                    (2..=width).contains(&layout.width)
                        && (layout.append_intercept
                            || (request.absorbed_terms.len() == 1
                                && (1..=3)
                                    .contains(&request.absorbed_terms[0].slope_sources.len())))
                }) {
                    if let Some(elements) = n.checked_mul(layout.width) {
                        if let Some(bytes) = elements.checked_mul(8).filter(|bytes| {
                            *bytes <= rhs_budget
                                && *bytes
                                    <= memory
                                        .remaining()
                                        .saturating_sub(2 * 4096 * width * 8 + 4096)
                        }) {
                            memory.admit(bytes as u128)?;
                            let mut rows = Vec::new();
                            rows.try_reserve_exact(elements)
                                .map_err(|_| allocation("unable to reserve numeric loaded RHS"))?;
                            if rows.capacity() == elements {
                                memory.retain(bytes as u128)?;
                                *rhs = Some(rows);
                                rhs_layout = Some(layout);
                            }
                        }
                    }
                }
            }
        }
    }
    let reserved_read = if defer_columns && all_sources_numeric {
        let mut positions = reserved_empty(numeric_width, "reserved numeric SPI positions")?;
        let mut columns = reserved_empty(numeric_width, "reserved numeric SPI columns")?;
        let mut missing = reserved_empty(numeric_width, "reserved numeric SPI missing columns")?;
        positions.push(outcome_position);
        columns.push(&mut outcome.values);
        missing.push(&mut outcome.missing);
        for ((_, loader), position) in source_loaders.iter_mut().zip(&source_positions) {
            let OlsSourceLoader::Numeric(column) = loader else {
                unreachable!("numeric sources checked");
            };
            positions.push(*position);
            columns.push(&mut column.values);
            missing.push(&mut column.missing);
        }
        for (position, column) in [
            (analytic_position, analytic_weight.as_mut()),
            (frequency_position, frequency_weight.as_mut()),
            (probability_position, probability_weight.as_mut()),
        ] {
            if let (Some(position), Some(column)) = (position, column) {
                positions.push(position);
                columns.push(&mut column.values);
                missing.push(&mut column.missing);
            }
        }
        let prepared_read = if let Some(hint) = load_hint.as_mut() {
            crate::numeric_pipeline::try_read_prepared_reserved(
                spi,
                request,
                &positions,
                n,
                &mut columns,
                &mut missing,
                team.expect("deferred input has a team"),
                memory,
                rhs.as_mut().zip(rhs_layout),
                hint,
            )?
        } else {
            false
        };
        prepared_read
            || crate::numeric_pipeline::try_read_reserved(
                spi,
                &positions,
                n,
                &mut columns,
                &mut missing,
                team.expect("deferred input has a team"),
                memory,
                rhs.as_mut().zip(rhs_layout),
            )?
    } else {
        false
    };
    if defer_columns && !reserved_read {
        // A rejected optional path has not read observations. Restore the
        // initialized arrays required by the ordinary loader before fallback.
        outcome.values.resize(n, 0.0);
        outcome.missing.resize(n, false);
        for (_, loader) in &mut source_loaders {
            if let OlsSourceLoader::Numeric(column) = loader {
                column.values.resize(n, 0.0);
                column.missing.resize(n, false);
            }
        }
        for column in [
            &mut analytic_weight,
            &mut frequency_weight,
            &mut probability_weight,
        ]
        .into_iter()
        .flatten()
        {
            column.values.resize(n, 0.0);
            column.missing.resize(n, false);
        }
    }
    if candidate_all && all_sources_numeric || n >= INPUT_CANDIDATE_BATCH_MIN_ROWS {
        if let Some(position) = candidate_position {
            for (row, retained) in candidate
                .explicit_mut()
                .expect("marker reads own explicit storage")
                .iter_mut()
                .enumerate()
            {
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
        if !reserved_read {
            read_numeric_columns_in_chunks(
                spi,
                &positions,
                &mut columns,
                &mut missing,
                (!candidate_all).then_some(candidate.view()),
            )?;
        }
        if candidate_all
            && all_sources_numeric
            && input_complete
            && (missing.iter().any(|flags| flags.iter().any(|flag| *flag))
                || load_hint
                    .as_ref()
                    .is_some_and(|target| target.has_missing()))
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
        for zero_based in 0..n {
            if zero_based % INPUT_INTERRUPT_POLL_STRIDE == 0 {
                spi.check_interrupted()?;
            }
            let observation = zero_based + 1;
            let retained_candidate = if let Some(candidate_position) = candidate_position {
                let retained = read_candidate_marker(spi, candidate_position, observation)?;
                candidate
                    .explicit_mut()
                    .expect("marker reads own explicit storage")[zero_based] = retained;
                retained
            } else {
                candidate[zero_based]
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
    candidate: Option<fereg_core::data::UniformValuesRef<'_, bool>>,
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
            if let Some(excluded) = candidate
                .slice(start..end)
                .iter()
                .position(|retained| !*retained)
            {
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
            Self::FixedWidthString {
                column,
                provisional_codes,
            } => {
                let value =
                    spi.read_string_with_limit(position, observation, memory.remaining())?;
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
            Self::FixedWidthString {
                mut column,
                provisional_codes,
            } => {
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

fn add_fitted_terms(
    prepared: &fereg_core::ols::PreparedOlsData,
    terms: &[(usize, f64)],
    output: &mut [f64],
) -> Result<()> {
    if !fereg_core::ols::add_numeric_ols_predictor(
        prepared.model(),
        prepared.sample(),
        terms,
        output,
    )? {
        output
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(row, value)| -> Result<()> {
                for &(column, coefficient) in terms {
                    *value = coefficient.mul_add(prepared.column_value(column, row)?, *value);
                }
                Ok(())
            })?;
    }
    Ok(())
}

fn certify_recovered_model_outputs(
    prepared: &'_ PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    fitted_terms: &[(usize, f64)],
    absorbed: &[f64],
    workspace: &mut [f64],
    tolerance: f64,
) -> Result<()> {
    if absorbed.len() != workspace.len() || workspace.len() != fit.residuals().len() {
        return Err(invalid("model recovery output dimensions disagree"));
    }
    workspace.fill(0.0);
    add_fitted_terms(prepared, fitted_terms, workspace)?;
    let mut error = 0.0_f64;
    let mut scale = 0.0_f64;
    for row in 0..workspace.len() {
        let y = prepared.sample().outcome[row];
        let xb = workspace[row];
        let fe = absorbed[row];
        let residual = fit.residuals()[row];
        let difference = y - ((xb + fe) + residual);
        if !difference.is_finite() {
            return Err(invalid_data(
                "saved FE original model reconstruction is nonfinite",
            ));
        }
        error = error.max(difference.abs());
        scale = scale.max(y.abs() + xb.abs() + fe.abs() + residual.abs());
    }
    if !scale.is_finite()
        || (scale == 0.0 && error != 0.0)
        || (scale > 0.0 && error / scale > tolerance)
    {
        return Err(invalid_data(
            "saved FE product failed independent original model reconstruction",
        ));
    }
    Ok(())
}

/// Derived observation outputs, on the action team inside the polled native
/// run so Break also reaches saved-FE recovery.
fn derive_observation_outputs(
    prepared: &PreparedOlsData,
    inference: CertifiedOlsInference<'_>,
    action: &EstimateAction,
) -> Result<DerivedObservationOutputs> {
    let categorical = prepared.uses_categorical_preparation().then_some(prepared);
    let known_connected =
        categorical.is_some() && normalization_known_connected(inference.absorbed_dof());
    build_derived_observation_outputs_in_team(
        prepared,
        categorical,
        inference.fit(),
        &action.outputs,
        known_connected,
        action.fit,
    )
}

fn build_derived_observation_outputs_in_team(
    prepared: &'_ PreparedOlsData,
    categorical: Option<&PreparedOlsData>,
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
    let started = Instant::now();
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
        add_fitted_terms(prepared, &fitted_terms, xb)?;
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
            add_fitted_terms(prepared, &fitted_terms, target)?;
            target.par_iter_mut().enumerate().for_each(|(row, value)| {
                *value = prepared.sample().outcome[row] - *value - fit.residuals()[row];
            });
        }
    }
    if absorbed_target
        .as_ref()
        .is_some_and(|values| values.iter().any(|value| !value.is_finite()))
    {
        return Err(invalid_data("derived OLS absorbed target is not finite"));
    }

    let target_seconds = started.elapsed().as_secs_f64();
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
        );
        let recovered = match recovered {
            Ok(recovered) => recovered,
            Err(error)
                if error.code == ErrorCode::InvalidData
                    && !categorical.has_absorbed_slope()
                    && error.message.starts_with(
                        "categorical recovery target is not certified to lie in the absorbed span:",
                    ) =>
            {
                let raw = absorbed_target
                    .as_mut()
                    .ok_or_else(|| invalid("model FE recovery target is absent"))?;
                raw.fill(0.0);
                add_fitted_terms(prepared, &fitted_terms, raw)?;
                let mut recovery_model_scale = 0.0_f64;
                for (row, value) in raw.iter_mut().enumerate() {
                    let y = prepared.sample().outcome[row];
                    recovery_model_scale = recovery_model_scale
                        .max(y.abs() + value.abs() + fit.residuals()[row].abs());
                    *value = y - *value;
                }
                let recovered = categorical.recover_model_effects(
                    raw,
                    fit.residuals(),
                    recovery_model_scale,
                    &mask,
                    known_connected,
                    true,
                    MapConfig {
                        tolerance: 1e-12,
                        max_iterations: 20_000,
                        check_every: 1,
                        symmetric: true,
                    },
                    recovery_budget,
                )?;
                certify_recovered_model_outputs(
                    prepared,
                    fit,
                    &fitted_terms,
                    recovered.absorbed_sum(),
                    raw,
                    1e-12,
                )?;
                recovered
            }
            Err(error) => return Err(error),
        };
        let (absorbed_sum, channels) = recovered.into_optional_parts();
        recovered_absorbed = if requested.absorbed.is_some() {
            absorbed_sum
        } else {
            None
        };
        saved_channels = channels;
    } else {
        saved_channels.resize_with(requested.saved_channels.len(), || None);
    }

    let recovery_seconds = started.elapsed().as_secs_f64() - target_seconds;
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
    if fereg_core::diagnostics::phase_timing_enabled() {
        eprintln!("FEREG_PHASE|ols_outputs|target={target_seconds:.9}|recovery={recovery_seconds:.9}|total={:.9}", started.elapsed().as_secs_f64());
    }
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
    let (memory_ledger, forecast) = action.memory_evidence(shadow)?;
    if std::env::var_os("FEREG_ACTION_MEMORY_AUDIT").is_some() {
        let resources = instrumentation::snapshot();
        crate::action_memory::emit(
            &memory_ledger,
            resources
                .allocation_peak_bytes
                .saturating_sub(observation.resources_start.allocation_current_bytes),
        )?;
    }
    if let Some(forecast) = forecast {
        forecast.trace(
            action.memory_baseline,
            instrumentation::native_live_before(),
        )?;
    }
    let memory_report = MemoryReport::estimated(
        action.memory_baseline,
        instrumentation::native_live_before(),
        memory_ledger.summary(),
        action.memory_plan.safety_headroom_bytes,
        forecast,
    )?;
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
    let n = shadow.prepared.sample().retained_rows().len();
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
        || shadow.prepared.sample().drop_reasons.len() != shadow.receipt[6] as usize
        || shadow.prepared.sample().retained_rows().len() != shadow.residuals.len()
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
    if invalid_shadow_observations(
        shadow.residuals,
        shadow.prepared,
        action.fit.execution.threads,
    )? {
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

fn invalid_shadow_observations(
    residuals: &[f64],
    prepared: &PreparedOlsData,
    threads: ThreadConfig,
) -> Result<bool> {
    // Membership and drop reasons belong to the immutable validated sample.
    // Publication still verifies the independently produced residual vector.
    if residuals.len() != prepared.sample().n_active() {
        return Ok(true);
    }
    let plan = fereg_core::parallel::StagePlan::new(
        residuals.len() as u128,
        residuals.len(),
        threads.resolved(),
    );
    if plan.lanes > 1 {
        use rayon::prelude::*;
        return ThreadTeam::new(threads).install(|| {
            residuals
                .par_iter()
                .with_min_len(plan.chunk(residuals.len()))
                .any(|value| !value.is_finite())
        });
    }
    Ok(residuals.iter().any(|value| !value.is_finite()))
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
        shadow.prepared.sample().drop_reasons.len() as f64,
        shadow.dof[5],
        shadow.prepared.sample().retained_rows().len() as f64,
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
    if candidate > shadow.prepared.sample().drop_reasons.len()
        || shadow.prepared.sample().retained_rows().len() > candidate
    {
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
            || !matches!(row[0], 1.0 | 2.0 | 3.0 | 4.0 | 5.0 | 6.0 | 7.0 | 8.0)
            || !matches!(row[1], 1.0 | 2.0 | 3.0 | 4.0 | 5.0 | 6.0 | 7.0 | 8.0)
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
    let sample = shadow.prepared.sample();
    // The sealed prepared owner proves agreement between the retained map
    // and drop reasons. A full sample keeps its initialized private marker;
    // without requested observation products there is no row store to perform.
    let batched = try_store_full_observation_outputs(action, shadow, derived, spi)?;
    let observation_rows = if batched
        || (sample.retained_rows().len() == sample.drop_reasons.len()
            && action.outputs.residual.is_none()
            && action.outputs.drop_reason.is_none()
            && action.outputs.xb.is_none()
            && action.outputs.xbd.is_none()
            && action.outputs.absorbed.is_none()
            && action.outputs.saved_channels.iter().all(Option::is_none))
    {
        0
    } else {
        sample.drop_reasons.len()
    };
    let mut retained_cursor = 0_usize;
    for (zero_based, reason) in sample
        .drop_reasons
        .iter()
        .copied()
        .enumerate()
        .take(observation_rows)
    {
        let observation = zero_based + 1;
        let retained = reason == fereg_core::DropReason::Retained;
        if !retained && reason != fereg_core::DropReason::ExcludedByIfIn {
            spi.store_numeric(action.outputs.sample, observation, 0.0)?;
        }
        if retained {
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
        (
            &action.matrices.omission_b_marker,
            1,
            dimension,
            &shadow.omission_b_marker[..],
        ),
        (
            &action.matrices.omission_v_marker,
            dimension,
            dimension,
            &shadow.omission_v_marker[..],
        ),
        (&action.matrices.fit, FIT_ROWS, 2, &shadow.fit[..]),
        (&action.matrices.dof, DOF_ROWS, 2, &shadow.dof[..]),
        (
            &action.matrices.solver,
            SOLVER_ROWS,
            SOLVER_COLUMNS,
            &shadow.solver[..],
        ),
        (
            &action.matrices.fixed_effects,
            shadow.fixed_effect_rows,
            FE_COLUMNS,
            &shadow.fixed_effects[..],
        ),
        (
            &action.matrices.clusters,
            shadow.cluster_rows,
            CLUSTER_COLUMNS,
            &shadow.clusters[..],
        ),
        (
            &action.matrices.subsets,
            shadow.subset_rows,
            SUBSET_COLUMNS,
            &shadow.subsets[..],
        ),
        (
            &action.matrices.certificates,
            CERTIFICATE_ROWS,
            2,
            &shadow.certificates[..],
        ),
        (&action.matrices.runtime, RUNTIME_ROWS, 2, runtime),
        (
            &action.matrices.memory_report,
            MEMORY_ROWS,
            2,
            &memory_report.0[..],
        ),
    ] {
        store_matrix(spi, name, rows, columns, values)?;
    }
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

fn try_store_full_observation_outputs(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
    derived: &DerivedObservationOutputs,
    spi: &mut dyn SpiApi,
) -> Result<bool> {
    // The sealed prepared owner proves an identity row map for a full sample. Preserve the scalar store
    // order using borrowed columns; partial samples keep their marker updates.
    if shadow.prepared.sample().retained_rows().len() != shadow.prepared.sample().drop_reasons.len()
        || shadow.prepared.sample().retained_rows().len() < 4096
        || action.outputs.drop_reason.is_some()
    {
        return Ok(false);
    }
    let mut columns = [(0_u32, &[] as &[f64]); crate::input::MAX_NUMERIC_STORE_COLUMNS];
    let mut count = 0;
    if let Some(position) = action.outputs.residual {
        columns[count] = (position, shadow.residuals);
        count += 1;
    }
    for (position, values) in [
        (action.outputs.xb, derived.xb.as_deref()),
        (action.outputs.xbd, derived.xbd.as_deref()),
        (action.outputs.absorbed, derived.absorbed.as_deref()),
    ] {
        if let (Some(position), Some(values)) = (position, values) {
            columns[count] = (position, values);
            count += 1;
        }
    }
    for (position, values) in action
        .outputs
        .saved_channels
        .iter()
        .zip(&derived.saved_channels)
    {
        if let (Some(position), Some(values)) = (*position, values.as_deref()) {
            if count == columns.len() {
                return Ok(false);
            }
            columns[count] = (position, values);
            count += 1;
        }
    }
    if count == 0 {
        return Ok(false);
    }
    spi.try_store_numeric_columns(&columns[..count])
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
    let mut names = BTreeSet::new();
    for name in input_matrix_names {
        if !names.insert(name.as_str()) {
            return Err(invalid("OLS input matrix name is duplicated"));
        }
    }
    for matrix in prepared_matrices.names() {
        let name = matrix.as_str();
        let (rows, columns) = (matrix.rows(), matrix.columns());
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

pub(crate) fn source_requires_numeric(request: &OlsRequestV1, source: SourceId) -> bool {
    request.model.terms.iter().any(|term| {
        term.components.iter().any(|component| {
            component.source == source && component.kind == ComponentKind::Continuous
        })
    }) || request
        .absorbed_terms
        .iter()
        .any(|term| term.slope_sources.contains(&source))
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

fn loaded_column_reservation(
    n: usize,
    label: &'static str,
    defer: bool,
) -> Result<LoadedNumericColumn> {
    if defer {
        Ok(LoadedNumericColumn {
            values: reserved_empty(n, label)?,
            missing: reserved_empty(n, label)?,
        })
    } else {
        empty_loaded_column(n, label)
    }
}

fn empty_loaded_column(n: usize, label: &'static str) -> Result<LoadedNumericColumn> {
    Ok(LoadedNumericColumn {
        values: reserved_filled(n, 0.0, label)?,
        missing: reserved_filled(n, false, label)?,
    })
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
    match value.to_ascii_lowercase().as_str() {
        "auto" => Ok(FeSolverKind::Auto),
        "map" => Ok(FeSolverKind::Map),
        "lsmr" => Ok(FeSolverKind::Lsmr),
        "cmg" => Ok(FeSolverKind::Cmg),
        "cmgschur" => Ok(FeSolverKind::CmgSchur),
        "cg" => Ok(FeSolverKind::Cg),
        "it" | "irons-tuck" => Ok(FeSolverKind::IronsTuck),
        "squarem" => Ok(FeSolverKind::Squarem),
        _ => Err(syntax(
            "solver must be auto, map, lsmr, cmg, cmgschur, cg, it, or squarem",
        )),
    }
}

fn parse_preconditioner(value: &str) -> Result<LsmrPreconditioner> {
    match value.to_ascii_lowercase().as_str() {
        "auto" => Ok(LsmrPreconditioner::Auto),
        "off" | "none" => Ok(LsmrPreconditioner::Off),
        "diagonal" => Ok(LsmrPreconditioner::Diagonal),
        "schwarz" | "additive" => Ok(LsmrPreconditioner::Schwarz),
        _ => Err(syntax(
            "preconditioner must be auto, off, diagonal, or schwarz",
        )),
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

fn protocol(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::ProtocolMismatch, message)
}

#[cfg(test)]
mod tests;
