use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::ppml::{
    CertifiedPpmlResult, PpmlAllocatorSnapshot, PpmlClusterDimensionResult, PpmlCoefficientStatus,
    PpmlDofReport, PpmlFixedEffectChannelKind, PpmlFixedEffectResult, PpmlMemoryRoute,
    PpmlPredictionKind, PpmlPsdState, PpmlResidualKind, PpmlTestDistribution, PpmlVceKind,
    PpmlVceRequest, PpmlVceResult, PreparedPpmlAdjustmentKind, PPML_PHASES,
};
use fereg_core::ppml_separation::{
    MethodStatus, SeparationCertificateProof, SeparationMethod, SeparationMethods, SeparationReport,
};
use fereg_core::protocol::{
    PpmlAdjustmentV1, PpmlResultReceiptV1, PpmlResultVceV1, PPML_RESULT_DROP_COUNT_ROWS,
    PPML_RESULT_INFERENCE_ROWS, PPML_RESULT_RECEIPT_COLUMNS, PPML_RESULT_SEPARATION_SEQUENCE_ROWS,
};
use fereg_core::{DropReason, FeSolverKind, MemoryMode, WeightKind};

pub(crate) const COEFFICIENT_STATUS_COLUMNS: usize = 4;
pub(crate) const FIT_ROWS: usize = 27;
pub(crate) const DOF_ROWS: usize = 12;
pub(crate) const FE_COLUMNS: usize = 8;
pub(crate) const CLUSTER_COLUMNS: usize = 4;
pub(crate) const SUBSET_COLUMNS: usize = 4;
pub(crate) const SEPARATION_COLUMNS: usize = 14;
pub(crate) const CERTIFICATE_ROWS: usize = 16;
pub(crate) const INFERENCE_ROWS: usize = PPML_RESULT_INFERENCE_ROWS;
pub(crate) const DROP_COUNT_ROWS: usize = PPML_RESULT_DROP_COUNT_ROWS;
pub(crate) const DROP_COUNT_COLUMNS: usize = 2;
pub(crate) const SEPARATION_SEQUENCE_ROWS: usize = PPML_RESULT_SEPARATION_SEQUENCE_ROWS;
pub(crate) const SEPARATION_SEQUENCE_COLUMNS: usize = 13;
pub(crate) const SOLVER_ROWS: usize = 4;
pub(crate) const SOLVER_COLUMNS: usize = 22;
/// Hidden PPML runtime wire. Rows 1--40 preserve the original additive 1.0
/// layout. Rows 41--108 carry the explicit convergence, warmed-allocation,
/// action-allocation, and per-phase peak evidence required for qualification.
pub(crate) const RUNTIME_ROWS: usize = 108;
const STATA_SYSTEM_MISSING: f64 = 8.988_465_674_311_58e307;

/// Values at or above Stata's system-missing threshold are finite Rust
/// numbers but are missing data to Stata. Scientific values must stay below
/// that boundary; the exact sentinel is reserved for explicitly optional
/// result cells.
pub(crate) fn is_stata_publishable(value: f64) -> bool {
    value.is_finite() && value.abs() < STATA_SYSTEM_MISSING
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StataCellClass {
    /// A finite-domain fitted value or scientific certificate which cannot be
    /// represented on Stata's numeric result wire.  ADR 0016 classifies this
    /// as numerical nonconvergence, not malformed user data.
    Scientific,
    /// Structural metadata or embedding telemetry.  Failure here is an
    /// internal identity/publication-contract violation.
    Structural,
}

/// Fully validated PPML publication shadow. Observation-scale products either
/// borrow the certified core result or are derived into owned buffers before
/// publication. Every small matrix is owned so publication does no scientific
/// computation and performs no allocation.
#[derive(Debug)]
pub(crate) struct PpmlShadowResult<'a> {
    pub(crate) bias: Option<crate::ppml_estimate::bias::Shadow<'a>>,
    pub(crate) b: Vec<f64>,
    pub(crate) covariance: Vec<f64>,
    pub(crate) coefficient_status: Vec<f64>,
    pub(crate) fit: Vec<f64>,
    pub(crate) dof: Vec<f64>,
    pub(crate) fixed_effects: Vec<f64>,
    pub(crate) clusters: Vec<f64>,
    pub(crate) subsets: Vec<f64>,
    pub(crate) separation: Vec<f64>,
    pub(crate) inference: Vec<f64>,
    pub(crate) drop_counts: Vec<f64>,
    pub(crate) separation_sequence: Vec<f64>,
    pub(crate) certificates: Vec<f64>,
    pub(crate) solver: Vec<f64>,
    pub(crate) runtime: Vec<f64>,
    pub(crate) receipt: [f64; PPML_RESULT_RECEIPT_COLUMNS],
    pub(crate) retained_original_rows: &'a [usize],
    pub(crate) drop_reasons: &'a [DropReason],
    pub(crate) residuals: &'a [f64],
    pub(crate) xb: &'a [f64],
    pub(crate) absorbed: &'a [f64],
    pub(crate) eta: &'a [f64],
    pub(crate) mean: &'a [f64],
    pub(crate) saved_channels: Vec<&'a [f64]>,
    pub(crate) coefficient_count: usize,
    pub(crate) fixed_effect_rows: usize,
    pub(crate) cluster_rows: usize,
    pub(crate) subset_rows: usize,
    pub(crate) separation_rows: usize,
    pub(crate) separation_execution_count: usize,
    pub(crate) separation_generation_count: usize,
    pub(crate) initial_generation_hash: u64,
    pub(crate) final_generation_hash: u64,
    pub(crate) separation_trace_digest: u64,
    pub(crate) generation_hash_digest: u64,
    pub(crate) requested_vce: PpmlVceRequest,
    pub(crate) adjustment_kind: PreparedPpmlAdjustmentKind,
    pub(crate) requested_memory: MemoryMode,
    pub(crate) requested_threads: usize,
    pub(crate) effective_threads: usize,
    pub(crate) planned_poolsize: usize,
    pub(crate) planned_preflight_finite_columns: usize,
    pub(crate) planned_observation_columns: usize,
    pub(crate) planned_publication_cells: usize,
    pub(crate) fixed_effect_metadata: &'a [PpmlFixedEffectResult],
    pub(crate) cluster_metadata: &'a [PpmlClusterDimensionResult],
}

impl PpmlShadowResult<'_> {
    /// Observation products and metadata borrow the certified result. Only
    /// matrix banks and the saved-channel descriptor bank own heap here.
    pub(crate) fn capacity_bytes(&self) -> u128 {
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
        ]
        .iter()
        .map(|values| values.capacity() as u128 * std::mem::size_of::<f64>() as u128)
        .sum::<u128>()
            + self.bias.as_ref().map_or(0, |b| b.capacity_bytes())
            + self.saved_channels.capacity() as u128 * std::mem::size_of::<&[f64]>() as u128
    }
}

/// The embedding publication frame starts after core certification returns.
/// The original embedding reserve conservatively retains its worker/input
/// allowance; the surviving action and certified result are explicit parents.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PpmlPublicationMemory {
    pub(crate) parent_bytes: u128,
    pub(crate) shadow_bytes: u128,
}

impl PpmlPublicationMemory {
    pub(crate) fn estimate(
        embedding_bytes: u128,
        action_bytes: u128,
        result_bytes: u128,
        matrix_cells: usize,
        declared_channels: usize,
    ) -> Result<Self> {
        let parent_bytes = embedding_bytes
            .max(action_bytes)
            .checked_add(result_bytes)
            .ok_or_else(|| allocation("PPML publication parent size overflows"))?;
        let shadow_bytes = (matrix_cells as u128)
            .checked_mul(std::mem::size_of::<f64>() as u128)
            .and_then(|bytes| {
                bytes.checked_add(declared_channels as u128 * std::mem::size_of::<&[f64]>() as u128)
            })
            .ok_or_else(|| allocation("PPML publication shadow size overflows"))?;
        let model = Self {
            parent_bytes,
            shadow_bytes,
        };
        model.peak_bytes()?;
        Ok(model)
    }

    pub(crate) fn peak_bytes(self) -> Result<u128> {
        self.parent_bytes
            .checked_add(self.shadow_bytes)
            .ok_or_else(|| allocation("PPML publication frame size overflows"))
    }

    pub(crate) fn admit(self, usable_bytes: Option<u128>) -> Result<()> {
        let peak = self.peak_bytes()?;
        if usable_bytes.is_some_and(|usable| peak > usable) {
            return Err(allocation(
                "PPML retained result and publication exceed usable memory",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_capacity(self, shadow: &PpmlShadowResult<'_>) -> Result<()> {
        if shadow.capacity_bytes() > self.shadow_bytes {
            return Err(allocation(
                "PPML publication capacities exceed preflight admission",
            ));
        }
        Ok(())
    }
}

/// Publication-only choices that are intentionally absent from the certified
/// scientific result. The result itself owns every model, row, inference,
/// topology, and telemetry identity used below.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CertifiedPpmlShadowContext {
    pub(crate) residual: Option<PpmlResidualKind>,
    pub(crate) xb_requested: bool,
    pub(crate) separation_methods: SeparationMethods,
    pub(crate) drop_singletons: bool,
    pub(crate) elapsed_seconds: f64,
}

/// Embedding-owned observations that become complete only after shadow
/// construction and every staged SPI store except RUNTIME and COMPLETE.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PpmlRuntimeFinalObservation {
    pub(crate) total_seconds: f64,
    pub(crate) publication_extension_seconds: f64,
    pub(crate) publication_peak_bytes: Option<u64>,
    pub(crate) action_start: Option<PpmlAllocatorSnapshot>,
    pub(crate) action_end: Option<PpmlAllocatorSnapshot>,
    pub(crate) previous_baseline_bytes: Option<u64>,
    pub(crate) modeled_selected_bytes: u128,
}

/// Patch only preallocated runtime cells after the final embedding snapshot.
/// The success path performs no allocation, preserving the action/phase peak
/// boundary immediately before RUNTIME and the absolute COMPLETE store.
pub(crate) fn finalize_ppml_runtime_shadow(
    runtime: &mut [f64],
    observation: PpmlRuntimeFinalObservation,
) -> Result<()> {
    if runtime.len() != RUNTIME_ROWS
        || !is_stata_publishable(observation.total_seconds)
        || observation.total_seconds < 0.0
        || !is_stata_publishable(observation.publication_extension_seconds)
        || observation.publication_extension_seconds < 0.0
    {
        return Err(invalid("PPML final runtime observation is invalid"));
    }
    runtime[0] = observation.total_seconds;
    let core_publication_seconds = if runtime[39] == STATA_SYSTEM_MISSING {
        0.0
    } else if is_stata_publishable(runtime[39]) && runtime[39] >= 0.0 {
        runtime[39]
    } else {
        return Err(invalid("PPML core publication timing is invalid"));
    };
    runtime[39] = core_publication_seconds + observation.publication_extension_seconds;
    if !is_stata_publishable(runtime[39]) {
        return Err(invalid(
            "PPML complete publication timing is not publishable",
        ));
    }

    for slot in &mut runtime[16..23] {
        *slot = STATA_SYSTEM_MISSING;
    }
    runtime[78] = STATA_SYSTEM_MISSING;
    for slot in &mut runtime[79..82] {
        *slot = STATA_SYSTEM_MISSING;
    }
    match (observation.action_start, observation.action_end) {
        (None, None) => {
            if observation.previous_baseline_bytes.is_some() {
                return Err(invalid(
                    "PPML previous allocator baseline lacks an action observation",
                ));
            }
        }
        (Some(start), Some(end)) => {
            if end.peak_bytes < start.current_bytes
                || end.peak_bytes < end.current_bytes
                || end.allocation_events < start.allocation_events
            {
                return Err(invalid("PPML final allocator observation is nonmonotone"));
            }
            runtime[17] = telemetry_u128(u128::from(start.current_bytes), "allocator start bytes")?;
            runtime[18] = telemetry_u128(u128::from(end.current_bytes), "allocator current bytes")?;
            runtime[19] = telemetry_u128(u128::from(end.peak_bytes), "allocator peak bytes")?;
            let peak_delta = end.peak_bytes - start.current_bytes;
            runtime[20] = telemetry_u128(u128::from(peak_delta), "allocator peak delta")?;
            runtime[21] = telemetry_i128(
                i128::from(end.current_bytes) - i128::from(start.current_bytes),
                "allocator signed growth",
            )?;
            if observation.modeled_selected_bytes > 0 {
                let ratio = peak_delta as f64 / observation.modeled_selected_bytes as f64;
                if !is_stata_publishable(ratio) || ratio < 0.0 {
                    return Err(invalid("PPML allocation/model ratio is invalid"));
                }
                runtime[22] = ratio;
            }
            runtime[79] = telemetry_u128(
                u128::from(start.allocation_events),
                "action allocator events start",
            )?;
            runtime[80] = telemetry_u128(
                u128::from(end.allocation_events),
                "action allocator events end",
            )?;
            runtime[81] = telemetry_u128(
                u128::from(end.allocation_events - start.allocation_events),
                "action allocator events delta",
            )?;
            if let Some(previous) = observation.previous_baseline_bytes {
                runtime[16] = telemetry_u128(u128::from(previous), "previous allocator baseline")?;
                runtime[78] = telemetry_i128(
                    i128::from(start.current_bytes) - i128::from(previous),
                    "allocator repeated-call baseline growth",
                )?;
            }
        }
        _ => return Err(invalid("PPML final allocator observation is partial")),
    }

    if let Some(peak) = observation.publication_peak_bytes {
        let peak = telemetry_u128(u128::from(peak), "observed PPML publication peak bytes")?;
        if runtime[107] == STATA_SYSTEM_MISSING || peak > runtime[107] {
            runtime[107] = peak;
        }
    }
    validate_final_runtime_cells(runtime)
}

fn validate_final_runtime_cells(runtime: &[f64]) -> Result<()> {
    for (index, value) in runtime.iter().copied().enumerate() {
        let optional = (16..=22).contains(&index)
            || (27..40).contains(&index)
            || (41..76).contains(&index)
            || index == 78
            || (79..82).contains(&index)
            || (95..108).contains(&index);
        if value == STATA_SYSTEM_MISSING && optional {
            continue;
        }
        if !is_stata_publishable(value) {
            return Err(invalid("PPML finalized runtime contains an invalid cell"));
        }
    }
    if !matches!(runtime[40], 0.0 | 1.0) || runtime[76] != 1.0 || runtime[77] != 1.0 {
        return Err(invalid(
            "PPML finalized runtime has invalid audit or convergence flags",
        ));
    }
    let audit_present = runtime[40] == 1.0;
    if (41..76).any(|index| (runtime[index] == STATA_SYSTEM_MISSING) == audit_present)
        || audit_present && (95..108).any(|index| runtime[index] == STATA_SYSTEM_MISSING)
    {
        return Err(invalid(
            "PPML finalized runtime has partial allocation-audit evidence",
        ));
    }
    for phase in 0..PPML_PHASES.len() {
        let modeled = runtime[82 + phase];
        let observed = runtime[95 + phase];
        if observed != STATA_SYSTEM_MISSING && observed > modeled {
            return Err(invalid(
                "PPML observed phase peak exceeds its preflight memory model",
            ));
        }
    }
    let action_present = runtime[17] != STATA_SYSTEM_MISSING;
    if [18_usize, 19, 20, 21, 22, 79, 80, 81]
        .into_iter()
        .any(|index| (runtime[index] != STATA_SYSTEM_MISSING) != action_present)
        || (runtime[16] == STATA_SYSTEM_MISSING) != (runtime[78] == STATA_SYSTEM_MISSING)
        || !action_present
            && (runtime[16] != STATA_SYSTEM_MISSING || runtime[78] != STATA_SYSTEM_MISSING)
    {
        return Err(invalid(
            "PPML finalized runtime has partial action allocator evidence",
        ));
    }
    Ok(())
}

/// Serialize one already-certified prepared PPML generation into a complete
/// private transaction shadow. No model reconstruction or independent sample
/// join occurs at this boundary.
pub(crate) fn build_certified_ppml_shadow(
    result: &CertifiedPpmlResult,
    context: CertifiedPpmlShadowContext,
) -> Result<PpmlShadowResult<'_>> {
    if !context.elapsed_seconds.is_finite() || context.elapsed_seconds < 0.0 {
        return Err(invalid("PPML transaction elapsed time is invalid"));
    }
    let coefficients = result.coefficients();
    let sample = result.sample();
    let inference = result.inference();
    let output = result.output();
    let convergence = result.convergence();
    let weights = result.weights();
    let coefficient_count = coefficients.values().len();
    if coefficient_count == 0 {
        return Err(invalid("certified PPML result has no coefficient slots"));
    }

    let b = fallible_copy(coefficients.values(), "PPML coefficient shadow")?;
    let covariance = fallible_copy(&inference.vce.covariance, "PPML covariance shadow")?;
    let mut coefficient_status = reserved_filled(
        checked_product(
            coefficient_count,
            COEFFICIENT_STATUS_COLUMNS,
            "PPML coefficient-status shadow",
        )?,
        0.0,
        "PPML coefficient-status shadow",
    )?;
    for (index, status) in coefficients.statuses().iter().copied().enumerate() {
        let start = index * COEFFICIENT_STATUS_COLUMNS;
        let identified = status == PpmlCoefficientStatus::Identified;
        coefficient_status[start] = exact_count(index + 1, "coefficient slot")?;
        coefficient_status[start + 1] = ppml_status_code(status);
        coefficient_status[start + 2] = f64::from(identified);
        coefficient_status[start + 3] = f64::from(!identified);
    }

    let likelihood = result.likelihood();
    let dof_report = &inference.dof;
    validate_dof_and_weight_identities(result)?;
    let weighted_outcome_sum = weighted_outcome_sum(sample.outcome(), sample.prior_weights())?;
    let mut fit = reserved_filled(FIT_ROWS, STATA_SYSTEM_MISSING, "PPML fit-statistics shadow")?;
    fit[0] = exact_count(weights.physical_observations, "physical observations")?;
    fit[1] = exact_count(dof_report.finite_within_rank, "finite rank")?;
    fit[2] = exact_count(dof_report.absorbed_rank, "absorbed rank")?;
    fit[3] = exact_count(dof_report.structural_residual, "structural residual DoF")?;
    fit[4] = exact_count(dof_report.reported, "reported DoF")?;
    fit[5] = likelihood.deviance;
    fit[6] = likelihood.log_likelihood;
    fit[7] = likelihood.null_log_likelihood;
    fit[8] = likelihood.pseudo_r_squared;
    fit[9] = likelihood.null_mean;
    if let Some(statistic) = inference.wald.statistic {
        fit[10] = statistic;
    }
    fit[11] = exact_count(inference.wald.degrees_of_freedom, "Wald DoF")?;
    fit[12] = weighted_outcome_sum;
    fit[13] = exact_count(convergence.outer_iterations, "outer iterations")?;
    fit[14] = exact_count(convergence.inner_iterations, "inner iterations")?;
    fit[15] = exact_count(convergence.step_halvings, "step halvings")?;
    fit[16] = exact_count(weights.effective_observations, "effective observations")?;
    fit[17] = weights.prepared_weight_sum;
    if let Some(raw_sum) = weights
        .raw_weight_sum
        .filter(|value| is_stata_publishable(*value))
    {
        fit[18] = raw_sum;
    }
    if let Some(multiplier) = weights
        .raw_to_prepared_multiplier
        .filter(|value| is_stata_publishable(*value))
    {
        fit[19] = multiplier;
    }
    fit[20] = weight_code(weights.kind);
    fit[21] = exact_count(sample.candidate_observations(), "candidate observations")?;
    fit[22] = exact_count(sample.original_observations(), "original observations")?;
    fit[23] = f64::from(convergence.converged);
    if let Some(log_sum) = weights.log_raw_weight_sum {
        fit[24] = log_sum;
    }
    fit[25] = weights.log_raw_to_prepared_multiplier;
    // All likelihood, score, and inference products use the canonical
    // prepared-weight experiment. No raw-scale objective is published.
    fit[26] = 1.0;

    let mut dof = reserved_filled(DOF_ROWS, 0.0, "PPML degrees-of-freedom shadow")?;
    dof[0] = exact_count(dof_report.effective_observations, "effective observations")?;
    dof[1] = exact_count(dof_report.finite_within_rank, "finite within rank")?;
    dof[2] = exact_count(dof_report.absorbed_rank, "absorbed rank")?;
    dof[3] = exact_count(dof_report.model_rank, "model rank")?;
    dof[4] = exact_count(dof_report.structural_residual, "structural residual DoF")?;
    dof[5] = exact_count(dof_report.reported, "reported DoF")?;
    dof[6] = dof_report
        .minimum_cluster_groups
        .map_or(Ok(STATA_SYSTEM_MISSING), |value| {
            exact_count(value, "minimum cluster groups")
        })?;
    dof[7] = exact_count(dof_report.connected_components, "connected components")?;
    dof[8] = f64::from(dof_report.absorbed_rank_is_approximate);
    dof[9] = exact_count(weights.physical_observations, "physical observations")?;
    dof[10] = exact_count(result.clusters().len(), "cluster dimensions")?;
    dof[11] = exact_count(
        dof_report
            .structural_residual
            .checked_sub(dof_report.reported)
            .ok_or_else(|| invalid("reported PPML DoF exceeds structural residual DoF"))?,
        "reported DoF cap",
    )?;

    let (fixed_effects, fixed_effect_rows) = build_fixed_effect_shadow(result)?;
    let (clusters, cluster_rows) = build_cluster_shadow(result)?;
    let (subsets, subset_rows) = build_subset_shadow(result)?;
    let (separation, separation_rows) =
        aggregate_separation(result.separation(), context.separation_methods)?;
    let inference_evidence = build_inference_evidence(result)?;
    let drop_counts = build_drop_count_evidence(result)?;
    let separation_sequence = build_separation_sequence_evidence(
        result.separation(),
        context.separation_methods,
        context.drop_singletons,
    )?;
    let initial_generation_hash = *result
        .separation()
        .generation_hashes
        .first()
        .ok_or_else(|| invalid("certified PPML separation lacks its initial generation hash"))?;
    let final_generation_hash = *result
        .separation()
        .generation_hashes
        .last()
        .ok_or_else(|| invalid("certified PPML separation lacks its final generation hash"))?;
    let separation_trace_digest = separation_trace_digest(result.separation())?;
    let generation_hash_digest = generation_hash_digest(result.separation())?;

    let score = &convergence.final_score;
    let reconstruction = output.certificate();
    let sandwich = inference.vce.certificate;
    let composite = result.certificate();
    let certificate_values = [
        score.finite_absolute_infinity_norm,
        score.fixed_effect_absolute_infinity_norm,
        score.relative_infinity_norm,
        score.relative_two_norm,
        convergence.linear_within_relative_infinity_norm,
        convergence.linear_reconstruction_relative_infinity_norm,
        reconstruction.predictor_relative_infinity_norm,
        reconstruction.mean_relative_infinity_norm,
        reconstruction.saved_total_relative_infinity_norm,
        reconstruction.saved_channels_relative_infinity_norm,
        sandwich.bread_symmetry_relative_infinity_norm,
        sandwich.meat_relative_infinity_norm,
        sandwich.covariance_relative_infinity_norm,
        sandwich.covariance_symmetry_relative_infinity_norm,
        sandwich.wald_solve_relative_infinity_norm,
        f64::from(composite.passed),
    ];
    debug_assert_eq!(certificate_values.len(), CERTIFICATE_ROWS);
    if certificate_values[..CERTIFICATE_ROWS - 1]
        .iter()
        .any(|value| !is_stata_publishable(*value))
    {
        return Err(numerical_nonconvergence(
            "certified PPML certificate values are not representable in Stata",
        ));
    }
    if certificate_values[..CERTIFICATE_ROWS - 1]
        .iter()
        .any(|value| *value < 0.0)
        || certificate_values[CERTIFICATE_ROWS - 1] != 1.0
    {
        return Err(invalid(
            "certified PPML certificate values are inconsistent",
        ));
    }
    let certificates = fallible_copy(&certificate_values, "PPML certificate shadow")?;
    let solver = build_solver_shadow(result)?;

    let runtime = build_runtime_shadow(result, context.elapsed_seconds)?;
    let residuals = context
        .residual
        .map(|kind| output.retained_prediction(residual_prediction_kind(kind)))
        .transpose()?
        .unwrap_or(&[]);
    let xb = if context.xb_requested {
        output.retained_prediction(PpmlPredictionKind::Xb)?
    } else {
        &[]
    };
    let mut saved_channels: Vec<&[f64]> = Vec::new();
    saved_channels
        .try_reserve_exact(result.fixed_effects().len())
        .map_err(|_| allocation("unable to reserve PPML saved-channel shadow"))?;
    let saved = output.saved_effects();
    for effect in result.fixed_effects() {
        let values = match (effect.saved_channel, saved) {
            (None, _) => &[],
            (Some(channel), Some(saved)) => saved
                .channels
                .get(channel)
                .ok_or_else(|| invalid("PPML saved channel is outside certified output"))?
                .as_slice(),
            (Some(_), None) => {
                return Err(invalid(
                    "PPML fixed-effect metadata requests an absent saved output",
                ));
            }
        };
        saved_channels.push(values);
    }

    let adjustment = match result.adjustment().input_kind {
        PreparedPpmlAdjustmentKind::None => PpmlAdjustmentV1::None,
        PreparedPpmlAdjustmentKind::Offset => PpmlAdjustmentV1::Offset,
        PreparedPpmlAdjustmentKind::Exposure => PpmlAdjustmentV1::Exposure,
    };
    let vce = match inference.vce.effective {
        PpmlVceKind::Robust => PpmlResultVceV1::Robust,
        PpmlVceKind::Clustered => PpmlResultVceV1::Clustered,
    };
    let receipt = PpmlResultReceiptV1 {
        vce,
        coefficient_count,
        covariance_dimension: coefficient_count,
        original_observations: sample.original_observations(),
        candidate_observations: sample.candidate_observations(),
        final_observations: sample.original_rows().len(),
        cluster_dimensions: cluster_rows,
        fixed_effect_rows,
        cluster_subset_rows: subset_rows,
        sample_present: true,
        residual_present: context.residual.is_some(),
        mandatory_certificates_passed: composite.passed,
        separated_observations: result.separation().separated_count,
        separation_report_rows: separation_rows,
        saved_fixed_effect_channels: output
            .saved_effects()
            .map_or(0, |saved| saved.channels.len()),
        adjustment,
        inference_rows: INFERENCE_ROWS,
        drop_count_rows: DROP_COUNT_ROWS,
        separation_sequence_rows: SEPARATION_SEQUENCE_ROWS,
        separation_execution_count: result.separation().methods.len(),
        separation_generation_count: result.separation().generations,
        initial_generation_hash,
        final_generation_hash,
        separation_trace_digest,
        generation_hash_digest,
    }
    .encode_incomplete()?;

    for (label, values) in [
        ("coefficients", b.as_slice()),
        ("covariance", covariance.as_slice()),
        ("certificates", certificates.as_slice()),
    ] {
        validate_stata_cells(label, values, &[], StataCellClass::Scientific)?;
    }
    for (label, values) in [
        ("coefficient status", coefficient_status.as_slice()),
        ("fixed effects", fixed_effects.as_slice()),
        ("clusters", clusters.as_slice()),
        ("cluster subsets", subsets.as_slice()),
        ("drop counts", drop_counts.as_slice()),
        ("separation sequence", separation_sequence.as_slice()),
    ] {
        validate_stata_cells(label, values, &[], StataCellClass::Structural)?;
    }
    validate_stata_cells(
        "inference evidence",
        &inference_evidence,
        &[],
        StataCellClass::Scientific,
    )?;
    validate_stata_cells("fit", &fit, &[10, 18, 19, 24], StataCellClass::Scientific)?;
    validate_stata_cells("degrees of freedom", &dof, &[6], StataCellClass::Structural)?;
    validate_stata_cells_where(
        "separation",
        &separation,
        |index| (8..12).contains(&(index % SEPARATION_COLUMNS)),
        StataCellClass::Scientific,
    )?;
    validate_stata_cells_where(
        "runtime",
        &runtime,
        |index| matches!(index, 16..=22 | 27..=39 | 41..=75 | 78..=81 | 95..=107),
        StataCellClass::Structural,
    )?;
    validate_stata_cells_where(
        "solver diagnostics",
        &solver,
        |index| (16..22).contains(&(index % SOLVER_COLUMNS)),
        StataCellClass::Structural,
    )?;
    for (label, values) in [
        ("residual", residuals),
        ("xb", xb),
        ("absorbed predictor", output.absorbed_predictor()),
        ("linear predictor", output.eta()),
        ("mean", output.mean()),
    ]
    .into_iter()
    .chain(
        saved_channels
            .iter()
            .copied()
            .map(|values| ("saved fixed effect", values)),
    ) {
        validate_stata_cells(label, values, &[], StataCellClass::Scientific)?;
    }

    Ok(PpmlShadowResult {
        bias: crate::ppml_estimate::bias::shadow(result)?,
        b,
        covariance,
        coefficient_status,
        fit,
        dof,
        fixed_effects,
        clusters,
        subsets,
        separation,
        inference: inference_evidence,
        drop_counts,
        separation_sequence,
        certificates,
        solver,
        runtime,
        receipt,
        retained_original_rows: sample.original_rows(),
        drop_reasons: sample.drop_reasons(),
        residuals,
        xb,
        absorbed: output.absorbed_predictor(),
        eta: output.eta(),
        mean: output.mean(),
        saved_channels,
        coefficient_count,
        fixed_effect_rows,
        cluster_rows,
        subset_rows,
        separation_rows,
        separation_execution_count: result.separation().methods.len(),
        separation_generation_count: result.separation().generations,
        initial_generation_hash,
        final_generation_hash,
        separation_trace_digest,
        generation_hash_digest,
        requested_vce: inference.vce.requested,
        adjustment_kind: result.adjustment().input_kind,
        requested_memory: result.memory().plan.requested,
        requested_threads: result.memory().requested_threads,
        effective_threads: result.memory().effective_threads,
        planned_poolsize: result.memory().plan.estimate.recommended_poolsize,
        planned_preflight_finite_columns: result.memory().plan.preflight_dimensions.finite_columns,
        planned_observation_columns: result.memory().plan.dimensions.requested_output_columns,
        planned_publication_cells: result.memory().plan.dimensions.publication_matrix_cells,
        fixed_effect_metadata: result.fixed_effects(),
        cluster_metadata: result.clusters(),
    })
}

fn build_solver_shadow(result: &CertifiedPpmlResult) -> Result<Vec<f64>> {
    let report = result.solver();
    let mut output = reserved_filled(
        checked_product(SOLVER_ROWS, SOLVER_COLUMNS, "PPML solver shadow")?,
        STATA_SYSTEM_MISSING,
        "PPML solver shadow",
    )?;
    for (row, engine) in report.engines().iter().enumerate() {
        let start = row * SOLVER_COLUMNS;
        output[start] = f64::from(report.requested.code());
        output[start + 1] = f64::from(report.selected.code());
        output[start + 2] = f64::from(report.selected_engine.code());
        output[start + 3] = f64::from(report.effective.code());
        output[start + 4] = f64::from(report.effective_engine.code());
        output[start + 5] = f64::from(report.checkpoint.code());
        output[start + 6] = f64::from(engine.engine.code());
        output[start + 7] = exact_count(engine.ordinary_frames, "solver ordinary frames")?;
        output[start + 8] = exact_count(engine.checkpoint_frames, "solver checkpoint frames")?;
        output[start + 9] = exact_count(engine.retries, "solver retries")?;
        output[start + 10] = exact_count(engine.fallback_count, "solver fallback count")?;
        output[start + 11] =
            solver_row_fallback_reason(engine.fallback_count, report.fallback_reason.code());
        output[start + 12] = exact_count(engine.iterations, "solver iterations")?;
        output[start + 13] = engine.setup_seconds;
        output[start + 14] = engine.solve_seconds;
        output[start + 15] = engine.certification_seconds;
        for (column, value) in [
            engine.absolute_infinity_norm_max,
            engine.relative_infinity_norm_max,
            engine.relative_two_norm_max,
            engine.normal_equation_max,
            engine.projection_absolute_infinity_norm_max,
            engine.backward_error_max,
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(value) = value {
                output[start + 16 + column] = value;
            }
        }
    }
    Ok(output)
}

fn solver_row_fallback_reason(fallback_count: usize, fallback_reason: u8) -> f64 {
    if fallback_count == 0 {
        0.0
    } else {
        f64::from(fallback_reason)
    }
}

fn build_fixed_effect_shadow(result: &CertifiedPpmlResult) -> Result<(Vec<f64>, usize)> {
    let rows = result.fixed_effects().len();
    let mut output = reserved_filled(
        checked_product(rows.max(1), FE_COLUMNS, "PPML fixed-effect shadow")?,
        0.0,
        "PPML fixed-effect shadow",
    )?;
    if rows == 0 {
        // OLS-compatible implicit-no-absorb sentinel.
        output[0] = 2.0;
        return Ok((output, 0));
    }
    for (row, effect) in result.fixed_effects().iter().enumerate() {
        if effect.redundant_parameters > effect.categories
            || effect.absorbed_rank != effect.categories - effect.redundant_parameters
        {
            return Err(invalid("PPML fixed-effect row rank identity is invalid"));
        }
        let (identity, source) = match effect.kind {
            PpmlFixedEffectChannelKind::Intercept if effect.source_ids.len() == 1 => {
                (1.0, f64::from(effect.source_ids[0]))
            }
            PpmlFixedEffectChannelKind::Intercept => (3.0, 0.0),
            PpmlFixedEffectChannelKind::Slope => (
                4.0,
                f64::from(
                    effect
                        .slope_source_id
                        .ok_or_else(|| invalid("PPML slope result lacks a source identity"))?,
                ),
            ),
        };
        let start = row * FE_COLUMNS;
        output[start..start + FE_COLUMNS].copy_from_slice(&[
            identity,
            f64::from(effect.term_id),
            source,
            exact_count(effect.categories, "fixed-effect categories")?,
            exact_count(effect.redundant_parameters, "fixed-effect redundancy")?,
            exact_count(effect.absorbed_rank, "fixed-effect rank")?,
            // The additive 1.0 wire reports whether the rank accounting is
            // approximate.  Core retains the logically complementary
            // positive certificate (`redundancy_exact`).
            f64::from(!effect.redundancy_exact),
            f64::from(effect.nested),
        ]);
    }
    Ok((output, rows))
}

fn build_cluster_shadow(result: &CertifiedPpmlResult) -> Result<(Vec<f64>, usize)> {
    let rows = result.clusters().len();
    let mut output = reserved_filled(
        checked_product(rows.max(1), CLUSTER_COLUMNS, "PPML cluster shadow")?,
        0.0,
        "PPML cluster shadow",
    )?;
    for (row, cluster) in result.clusters().iter().enumerate() {
        let start = row * CLUSTER_COLUMNS;
        output[start..start + CLUSTER_COLUMNS].copy_from_slice(&[
            exact_count(cluster.declared_dimension + 1, "cluster dimension")?,
            exact_count(cluster.source_ids.len(), "cluster source count")?,
            exact_count(cluster.groups, "cluster groups")?,
            0.0,
        ]);
    }
    Ok((output, rows))
}

fn validate_cluster_subsets(result: &CertifiedPpmlResult) -> Result<()> {
    let inference = result.inference();
    let dimensions = result.clusters().len();
    let expected_rows = if dimensions == 0 {
        0
    } else {
        (1_usize << dimensions) - 1
    };
    let expected_kind = if dimensions == 0 {
        PpmlVceKind::Robust
    } else {
        PpmlVceKind::Clustered
    };
    if inference.vce.effective != expected_kind
        || inference.vce.cluster_subsets.len() != expected_rows
        || inference.vce.cluster_dimension_groups.len() != dimensions
    {
        return Err(invalid(
            "PPML covariance topology does not match the declared clusters",
        ));
    }
    let expected_requested = match inference.vce.requested {
        PpmlVceRequest::Unadjusted | PpmlVceRequest::Robust => dimensions == 0,
        PpmlVceRequest::Clustered => dimensions > 0,
    };
    if !expected_requested
        || inference.vce.coefficient_distribution != PpmlTestDistribution::StandardNormal
        || inference.wald.distribution != PpmlTestDistribution::ChiSquared
    {
        return Err(invalid(
            "PPML covariance request or reference distribution is inconsistent",
        ));
    }
    let expected_correction = if dimensions == 0 {
        let observations = inference.dof.effective_observations;
        if observations < 2 {
            return Err(invalid(
                "robust PPML covariance requires at least two effective observations",
            ));
        }
        observations as f64 / (observations - 1) as f64
    } else {
        let minimum = *inference
            .vce
            .cluster_dimension_groups
            .iter()
            .min()
            .ok_or_else(|| invalid("clustered PPML covariance lacks group counts"))?;
        if minimum < 2 {
            return Err(invalid(
                "clustered PPML covariance requires at least two groups per dimension",
            ));
        }
        minimum as f64 / (minimum - 1) as f64
    };
    if !is_stata_publishable(inference.vce.correction) {
        return Err(numerical_nonconvergence(
            "certified PPML covariance correction is not representable in Stata",
        ));
    }
    if relative_error(inference.vce.correction, expected_correction) > 64.0 * f64::EPSILON {
        return Err(invalid("PPML covariance correction is inconsistent"));
    }
    for (row, subset) in inference.vce.cluster_subsets.iter().enumerate() {
        let cardinality = subset.mask.count_ones() as usize;
        let expected_mask = canonical_subset_mask(dimensions, row)?;
        if subset.mask != expected_mask
            || cardinality == 0
            || cardinality != subset.dimensions.len()
            || subset.sign != if cardinality % 2 == 1 { 1 } else { -1 }
            || subset.groups < 2
            || subset
                .dimensions
                .iter()
                .enumerate()
                .any(|(position, dimension)| {
                    *dimension >= dimensions
                        || (position > 0 && subset.dimensions[position - 1] >= *dimension)
                        || subset.mask & (1_u16 << *dimension) == 0
                })
        {
            return Err(invalid(
                "PPML cluster subsets are incomplete or not in canonical order",
            ));
        }
        for dimension in 0..dimensions {
            if (subset.mask & (1_u16 << dimension) != 0)
                != subset.dimensions.binary_search(&dimension).is_ok()
            {
                return Err(invalid(
                    "PPML cluster subset mask disagrees with its dimensions",
                ));
            }
        }
        if cardinality == 1 {
            let dimension = subset.dimensions[0];
            if subset.groups != inference.vce.cluster_dimension_groups[dimension]
                || subset.groups != result.clusters()[dimension].groups
            {
                return Err(invalid(
                    "PPML singleton cluster subset has the wrong group count",
                ));
            }
        }
    }
    Ok(())
}

fn canonical_subset_mask(dimensions: usize, row: usize) -> Result<u16> {
    let mut position = 0_usize;
    for cardinality in 1..=dimensions {
        for mask in 1_u16..(1_u16 << dimensions) {
            if mask.count_ones() as usize == cardinality {
                if position == row {
                    return Ok(mask);
                }
                position = position
                    .checked_add(1)
                    .ok_or_else(|| allocation("PPML subset inventory overflows"))?;
            }
        }
    }
    Err(invalid(
        "PPML subset row is outside the canonical inventory",
    ))
}

fn build_subset_shadow(result: &CertifiedPpmlResult) -> Result<(Vec<f64>, usize)> {
    let reports = &result.inference().vce.cluster_subsets;
    validate_cluster_subsets(result)?;
    let rows = reports.len();
    let mut output = reserved_filled(
        checked_product(rows.max(1), SUBSET_COLUMNS, "PPML subset shadow")?,
        0.0,
        "PPML subset shadow",
    )?;
    for (row, subset) in reports.iter().enumerate() {
        let start = row * SUBSET_COLUMNS;
        output[start..start + SUBSET_COLUMNS].copy_from_slice(&[
            f64::from(subset.mask),
            exact_count(subset.groups, "cluster subset groups")?,
            f64::from(subset.sign),
            exact_count(subset.dimensions.len(), "cluster subset cardinality")?,
        ]);
    }
    Ok((output, rows))
}

fn build_runtime_shadow(result: &CertifiedPpmlResult, elapsed_seconds: f64) -> Result<Vec<f64>> {
    validate_memory_telemetry(result)?;
    let memory = result.memory();
    let convergence = result.convergence();
    let plan = memory.plan;
    let mut runtime = reserved_filled(RUNTIME_ROWS, STATA_SYSTEM_MISSING, "PPML runtime shadow")?;
    runtime[0] = elapsed_seconds;
    runtime[1] = exact_count(convergence.outer_iterations, "outer iterations")?;
    runtime[2] = exact_count(convergence.inner_iterations, "inner iterations")?;
    runtime[3] = exact_count(convergence.step_halvings, "step halvings")?;
    runtime[4] = memory_mode_code(plan.requested);
    runtime[5] = memory_route_code(plan.selected);
    runtime[6] =
        linear_route_code(memory.linear_route.ok_or_else(|| {
            invalid("certified PPML result lacks the observed final linear route")
        })?);
    runtime[7] = f64::from(memory.fallback_mask);
    runtime[8] = exact_count(memory.fallback_count, "fallback count")?;
    runtime[9] = exact_count(memory.requested_threads, "requested threads")?;
    runtime[10] = exact_count(memory.effective_threads, "effective threads")?;
    runtime[11] = telemetry_u128(plan.estimate.fast.peak_bytes, "modeled fast bytes")?;
    runtime[12] = telemetry_u128(plan.estimate.low.peak_bytes, "modeled low bytes")?;
    runtime[13] = telemetry_u128(plan.selected_bytes(), "modeled selected bytes")?;
    runtime[14] = telemetry_u128(
        plan.selected_inventory().retained_within_scratch_bytes,
        "modeled scratch bytes",
    )?;
    runtime[15] = telemetry_u128(
        u128::from(
            memory
                .scratch_peak_bytes
                .ok_or_else(|| invalid("certified PPML result lacks observed scratch usage"))?,
        ),
        "observed scratch bytes",
    )?;
    if let Some(value) = memory.allocations.previous_baseline_bytes {
        runtime[16] = telemetry_u128(u128::from(value), "previous allocator baseline")?;
    }
    if let Some(value) = memory.allocations.start_bytes {
        runtime[17] = telemetry_u128(u128::from(value), "allocator start bytes")?;
    }
    if let Some(value) = memory.allocations.current_bytes {
        runtime[18] = telemetry_u128(u128::from(value), "allocator current bytes")?;
    }
    if let Some(value) = memory.allocations.peak_bytes {
        runtime[19] = telemetry_u128(u128::from(value), "allocator peak bytes")?;
    }
    if let Some(value) = memory.allocations.peak_delta_bytes() {
        runtime[20] = telemetry_u128(u128::from(value), "allocator peak delta")?;
    }
    if let Some(value) = memory.allocations.growth_bytes() {
        runtime[21] = telemetry_i128(value, "allocator signed growth")?;
    }
    if let Some(value) = memory.allocation_model_ratio() {
        runtime[22] = value;
    }
    runtime[23] = f64::from(memory.ppml_workspace_allocation_free);
    runtime[24] = f64::from(memory.whole_iteration_allocation_free);
    runtime[25] = exact_count(
        memory.diagnostic_records_retained,
        "retained diagnostic records",
    )?;
    runtime[26] = exact_count(
        convergence.strict_profile_rejections,
        "strict-profile rejections",
    )?;
    for (index, phase) in PPML_PHASES.iter().copied().enumerate() {
        if let Some(seconds) = memory.timings.seconds(phase) {
            runtime[27 + index] = seconds;
        }
    }
    if let Some(certificate) = memory.steady_state_allocation_certificate {
        runtime[40] = 1.0;
        runtime[41] = exact_count(
            certificate.measured_iterations,
            "allocation-audit measured iterations",
        )?;
        runtime[42] = telemetry_u128(
            u128::from(certificate.allocator_events_start),
            "allocation-audit allocator events start",
        )?;
        runtime[43] = telemetry_u128(
            u128::from(certificate.allocator_events_end),
            "allocation-audit allocator events end",
        )?;
        runtime[44] = telemetry_u128(
            u128::from(certificate.allocation_events),
            "allocation-audit allocator events delta",
        )?;
        runtime[45] = telemetry_u128(
            u128::from(certificate.allocator_current_start),
            "allocation-audit allocator current start",
        )?;
        runtime[46] = telemetry_u128(
            u128::from(certificate.allocator_current_end),
            "allocation-audit allocator current end",
        )?;
        runtime[47] = telemetry_i128(
            certificate.allocator_growth_bytes,
            "allocation-audit allocator current growth",
        )?;
        runtime[48] = telemetry_u128(
            u128::from(certificate.allocator_peak_start),
            "allocation-audit allocator peak start",
        )?;
        runtime[49] = telemetry_u128(
            u128::from(certificate.allocator_peak_end),
            "allocation-audit allocator peak end",
        )?;
        runtime[50] = telemetry_u128(
            u128::from(certificate.allocator_peak_growth_bytes),
            "allocation-audit allocator peak growth",
        )?;
        runtime[51] = exact_count(
            certificate.workspace_start.observation_capacity_sum,
            "allocation-audit PPML observation capacity start",
        )?;
        runtime[52] = exact_count(
            certificate.workspace_start.coefficient_capacity_sum,
            "allocation-audit PPML coefficient capacity start",
        )?;
        runtime[53] = exact_count(
            certificate.workspace_start.diagnostic_capacity,
            "allocation-audit PPML diagnostic capacity start",
        )?;
        runtime[54] = exact_count(
            certificate.workspace_end.observation_capacity_sum,
            "allocation-audit PPML observation capacity end",
        )?;
        runtime[55] = exact_count(
            certificate.workspace_end.coefficient_capacity_sum,
            "allocation-audit PPML coefficient capacity end",
        )?;
        runtime[56] = exact_count(
            certificate.workspace_end.diagnostic_capacity,
            "allocation-audit PPML diagnostic capacity end",
        )?;
        runtime[57] = f64::from(certificate.workspace_capacity_stable);
        runtime[58] = telemetry_u128(
            certificate.linear_workspace_start.element_capacity_sum,
            "allocation-audit linear element capacity start",
        )?;
        runtime[59] = exact_count(
            certificate
                .linear_workspace_start
                .retained_resident_elements,
            "allocation-audit linear resident elements start",
        )?;
        runtime[60] = telemetry_u128(
            u128::from(certificate.linear_workspace_start.retained_scratch_bytes),
            "allocation-audit linear retained scratch start",
        )?;
        runtime[61] = telemetry_u128(
            certificate
                .linear_workspace_start
                .fe_topology_resident_bytes,
            "allocation-audit linear FE topology bytes start",
        )?;
        runtime[62] = telemetry_u128(
            certificate.linear_workspace_end.element_capacity_sum,
            "allocation-audit linear element capacity end",
        )?;
        runtime[63] = exact_count(
            certificate.linear_workspace_end.retained_resident_elements,
            "allocation-audit linear resident elements end",
        )?;
        runtime[64] = telemetry_u128(
            u128::from(certificate.linear_workspace_end.retained_scratch_bytes),
            "allocation-audit linear retained scratch end",
        )?;
        runtime[65] = telemetry_u128(
            certificate.linear_workspace_end.fe_topology_resident_bytes,
            "allocation-audit linear FE topology bytes end",
        )?;
        runtime[66] = f64::from(certificate.linear_workspace_capacity_stable);
        runtime[67] = telemetry_u128(
            u128::from(certificate.scratch_bytes_start),
            "allocation-audit scratch bytes start",
        )?;
        runtime[68] = telemetry_u128(
            u128::from(certificate.scratch_bytes_end),
            "allocation-audit scratch bytes end",
        )?;
        runtime[69] = f64::from(certificate.scratch_stable);
        runtime[70] = linear_route_code(certificate.linear_route);
        runtime[71] = f64::from(certificate.route_allocation_qualified);
        runtime[72] = f64::from(certificate.linear_session_allocation_free);
        runtime[73] = f64::from(certificate.fresh_warm_equivalent);
        runtime[74] = telemetry_u128(
            certificate.steady_iteration_median_nanoseconds,
            "allocation-audit median steady iteration nanoseconds",
        )?;
        runtime[75] = f64::from(certificate.passed);
    } else {
        runtime[40] = 0.0;
    }
    runtime[76] = f64::from(convergence.two_consecutive_convergence);
    runtime[77] = f64::from(convergence.rank_frozen);
    if let (Some(start), Some(previous)) = (
        memory.allocations.start_bytes,
        memory.allocations.previous_baseline_bytes,
    ) {
        runtime[78] = telemetry_i128(
            i128::from(start) - i128::from(previous),
            "allocator repeated-call baseline growth",
        )?;
    }
    if let Some(value) = memory.allocations.allocation_events_start {
        runtime[79] = telemetry_u128(u128::from(value), "action allocator events start")?;
    }
    if let Some(value) = memory.allocations.allocation_events_end {
        runtime[80] = telemetry_u128(u128::from(value), "action allocator events end")?;
    }
    if let Some(value) = memory.allocations.allocation_events() {
        runtime[81] = telemetry_u128(u128::from(value), "action allocator events delta")?;
    }
    for (index, phase) in PPML_PHASES.iter().copied().enumerate() {
        runtime[82 + index] = telemetry_u128(
            plan.modeled_phase_peak_bytes(phase),
            "modeled PPML phase peak bytes",
        )?;
        if let Some(bytes) = memory.observed_phase_peak(phase) {
            runtime[95 + index] =
                telemetry_u128(u128::from(bytes), "observed PPML phase peak bytes")?;
        }
    }
    Ok(runtime)
}

fn validate_memory_telemetry(result: &CertifiedPpmlResult) -> Result<()> {
    let memory = result.memory();
    let route = memory
        .linear_route
        .ok_or_else(|| invalid("certified PPML result lacks the final linear route"))?;
    if route == FeSolverKind::Auto {
        return Err(invalid(
            "PPML memory telemetry has an unresolved linear route",
        ));
    }
    if memory.scratch_peak_bytes.is_none() {
        return Err(invalid(
            "PPML memory telemetry lacks observed scratch usage",
        ));
    }
    if memory.effective_threads == 0 || memory.plan.dimensions.threads != memory.effective_threads {
        return Err(invalid(
            "PPML memory telemetry has inconsistent thread counts",
        ));
    }
    if memory.fallback_mask & !0b1_1111 != 0
        || memory.fallback_count < memory.fallback_mask.count_ones() as usize
        || (memory.fallback_mask == 0) != (memory.fallback_count == 0)
    {
        return Err(invalid("PPML memory telemetry has inconsistent fallbacks"));
    }
    let workspace_capacity = memory
        .workspace_capacity
        .ok_or_else(|| invalid("PPML memory telemetry lacks its workspace capacity"))?;
    // Bounded iteration reports live in their own memory-plan inventory. The
    // steady-state PPML iteration fingerprint intentionally excludes them so
    // it can compare only buffers touched by every weighted iteration.
    if workspace_capacity.diagnostic_capacity != 0 {
        return Err(invalid(
            "PPML steady-state workspace capacity unexpectedly includes diagnostics",
        ));
    }
    let allocations = memory.allocations;
    match (
        allocations.start_bytes,
        allocations.current_bytes,
        allocations.peak_bytes,
    ) {
        (None, None, None) => {}
        (Some(start), Some(current), Some(peak)) if peak >= start && peak >= current => {}
        _ => {
            return Err(invalid(
                "PPML allocator telemetry is partial or nonmonotone",
            ))
        }
    }
    match (
        allocations.allocation_events_start,
        allocations.allocation_events_end,
    ) {
        (None, None) if allocations.start_bytes.is_none() => {}
        (Some(start), Some(end)) if allocations.start_bytes.is_some() && end >= start => {}
        _ => {
            return Err(invalid(
                "PPML action allocator event telemetry is partial or nonmonotone",
            ));
        }
    }
    match memory.steady_state_allocation_certificate {
        None if memory.whole_iteration_allocation_free => {
            return Err(invalid(
                "PPML whole-iteration allocation claim lacks its certificate",
            ));
        }
        Some(certificate)
            if !memory
                .plan
                .audit_route_matches_final(certificate.linear_route, route)
                || certificate.workspace_capacity_stable
                    != (certificate.workspace_start == certificate.workspace_end)
                || certificate.scratch_stable
                    != (certificate.scratch_bytes_start == certificate.scratch_bytes_end)
                || certificate.allocation_events
                    != certificate
                        .allocator_events_end
                        .checked_sub(certificate.allocator_events_start)
                        .ok_or_else(|| {
                            invalid("PPML allocator audit events are not monotone")
                        })?
                || certificate.allocator_growth_bytes
                    != i128::from(certificate.allocator_current_end)
                        - i128::from(certificate.allocator_current_start)
                || certificate.allocator_peak_growth_bytes
                    != certificate
                        .allocator_peak_end
                        .checked_sub(certificate.allocator_peak_start)
                        .ok_or_else(|| {
                            invalid("PPML allocator audit peaks are not monotone")
                        })?
                || certificate.passed != memory.whole_iteration_allocation_free
                || certificate.passed
                    && (!memory.ppml_workspace_allocation_free
                        || !certificate.workspace_capacity_stable
                        || !certificate.linear_workspace_capacity_stable
                        || !certificate.scratch_stable
                        || !certificate.route_allocation_qualified
                        || !certificate.linear_session_allocation_free
                        || !certificate.fresh_warm_equivalent) =>
        {
            return Err(invalid(
                "PPML warmed allocation certificate is inconsistent",
            ));
        }
        Some(_) => {}
        None => {}
    }
    Ok(())
}

const fn residual_prediction_kind(kind: PpmlResidualKind) -> PpmlPredictionKind {
    match kind {
        PpmlResidualKind::Response => PpmlPredictionKind::Response,
        PpmlResidualKind::Score => PpmlPredictionKind::Score,
        PpmlResidualKind::Pearson => PpmlPredictionKind::Pearson,
        PpmlResidualKind::Working => PpmlPredictionKind::Working,
        PpmlResidualKind::Deviance => PpmlPredictionKind::Deviance,
        PpmlResidualKind::Anscombe => PpmlPredictionKind::Anscombe,
    }
}

fn build_inference_evidence(result: &CertifiedPpmlResult) -> Result<Vec<f64>> {
    let inference = result.inference();
    encode_inference_evidence(
        &inference.vce,
        &inference.dof,
        result.coefficients().kept_full_slots().len(),
    )
}

fn encode_inference_evidence(
    vce: &PpmlVceResult,
    dof: &PpmlDofReport,
    identified_rank: usize,
) -> Result<Vec<f64>> {
    let psd = vce.psd;
    let requested_code = match vce.requested {
        PpmlVceRequest::Unadjusted => 1.0,
        PpmlVceRequest::Robust => 2.0,
        PpmlVceRequest::Clustered => 3.0,
    };
    let (effective_code, correction_policy, correction_authority) = match vce.effective {
        PpmlVceKind::Robust => {
            let observations = dof.effective_observations;
            if observations < 2 {
                return Err(invalid(
                    "robust PPML inference lacks two effective observations",
                ));
            }
            (2.0, 1.0, observations)
        }
        PpmlVceKind::Clustered => {
            let groups = dof
                .minimum_cluster_groups
                .ok_or_else(|| invalid("clustered PPML inference lacks its minimum group count"))?;
            if groups < 2 {
                return Err(invalid(
                    "clustered PPML inference lacks two groups per dimension",
                ));
            }
            (3.0, 2.0, groups)
        }
    };
    let expected_correction = correction_authority as f64 / (correction_authority - 1) as f64;
    if relative_error(vce.correction, expected_correction) > 16.0 * f64::EPSILON {
        return Err(invalid(
            "PPML inference evidence has an inconsistent covariance correction",
        ));
    }
    if vce.coefficient_distribution != PpmlTestDistribution::StandardNormal {
        return Err(invalid(
            "PPML inference evidence has an unsupported coefficient distribution",
        ));
    }
    let psd_state = match psd.state {
        PpmlPsdState::Strict => 1.0,
        PpmlPsdState::MultiwayRaw => 2.0,
        PpmlPsdState::MultiwayClipped => 3.0,
    };
    let certificate_tolerance = vce.certificate.tolerance;
    let certificate_errors = [
        psd.eigensystem_reconstruction_relative_error,
        psd.orthogonality_relative_error,
        psd.clipped_reconstruction_relative_error,
    ];
    if !certificate_tolerance.is_finite()
        || certificate_tolerance <= 0.0
        || certificate_errors
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0 || *value > certificate_tolerance)
        || !psd.raw_minimum_eigenvalue.is_finite()
        || !psd.final_minimum_eigenvalue.is_finite()
        || !psd.passed
        || psd.final_rank > identified_rank
        || psd.clipped_eigenvalues > identified_rank
        || psd.adjusted != (psd.state == PpmlPsdState::MultiwayClipped)
        || psd.adjusted != (psd.clipped_eigenvalues > 0)
        || (psd.state == PpmlPsdState::Strict && psd.clipped_eigenvalues != 0)
        || (psd.state == PpmlPsdState::MultiwayRaw
            && (psd.raw_minimum_eigenvalue < 0.0 || psd.clipped_eigenvalues != 0))
        || (psd.state == PpmlPsdState::MultiwayClipped
            && (psd.raw_minimum_eigenvalue >= 0.0 || psd.clipped_eigenvalues == 0))
    {
        return Err(invalid(
            "PPML inference evidence has inconsistent PSD diagnostics",
        ));
    }

    let values = [
        requested_code,
        effective_code,
        vce.correction,
        correction_policy,
        1.0,
        psd_state,
        f64::from(psd.adjusted),
        exact_count(psd.clipped_eigenvalues, "clipped PSD eigenvalues")?,
        psd.raw_minimum_eigenvalue,
        psd.final_minimum_eigenvalue,
        exact_count(psd.final_rank, "final PSD rank")?,
        psd.eigensystem_reconstruction_relative_error,
        psd.orthogonality_relative_error,
        psd.clipped_reconstruction_relative_error,
        certificate_tolerance,
        f64::from(psd.passed),
    ];
    debug_assert_eq!(values.len(), INFERENCE_ROWS);
    fallible_copy(&values, "PPML inference evidence shadow")
}

fn build_drop_count_evidence(result: &CertifiedPpmlResult) -> Result<Vec<f64>> {
    let sample = result.sample();
    let mut counts = [0_usize; DROP_COUNT_ROWS];
    for reason in sample.drop_reasons() {
        let slot = usize::from(reason.code());
        let count = counts
            .get_mut(slot)
            .ok_or_else(|| invalid("certified PPML sample has an unknown drop reason"))?;
        *count = checked_count_add(*count, 1, "PPML drop-reason count")?;
    }
    let total = counts.iter().try_fold(0_usize, |total, count| {
        checked_count_add(total, *count, "PPML drop-reason total")
    })?;
    let separated = counts[usize::from(DropReason::FixedEffectSeparation.code())
        ..=usize::from(DropReason::MuSeparation.code())]
        .iter()
        .try_fold(0_usize, |total, count| {
            checked_count_add(total, *count, "PPML separated-reason total")
        })?;
    let prepared = counts[usize::from(DropReason::Retained.code())]
        .checked_add(counts[usize::from(DropReason::Singleton.code())])
        .and_then(|value| value.checked_add(separated))
        .ok_or_else(|| allocation("PPML prepared-sample reason count overflows"))?;
    if total != sample.original_observations()
        || counts[usize::from(DropReason::Retained.code())] != sample.original_rows().len()
        || counts[usize::from(DropReason::ExcludedByIfIn.code())]
            != sample
                .original_observations()
                .saturating_sub(sample.candidate_observations())
        || counts[usize::from(DropReason::Singleton.code())] != result.separation().singleton_count
        || separated != result.separation().separated_count
        || prepared != result.separation().candidate_observations
        || prepared != sample.prepared_original_rows().len()
    {
        return Err(invalid(
            "PPML drop-reason evidence disagrees with the certified sample ledger",
        ));
    }
    let mut output = reserved_filled(
        checked_product(
            DROP_COUNT_ROWS,
            DROP_COUNT_COLUMNS,
            "PPML drop-count evidence",
        )?,
        0.0,
        "PPML drop-count evidence",
    )?;
    for (reason, count) in counts.into_iter().enumerate() {
        let start = reason * DROP_COUNT_COLUMNS;
        output[start] = exact_count(reason, "drop-reason code")?;
        output[start + 1] = exact_count(count, "drop-reason count")?;
    }
    Ok(output)
}

fn build_separation_sequence_evidence(
    report: &SeparationReport,
    enabled: SeparationMethods,
    drop_singletons: bool,
) -> Result<Vec<f64>> {
    let requested = [
        drop_singletons,
        enabled.fixed_effects,
        enabled.simplex,
        enabled.iterated_rectifier,
        enabled.mu,
    ];
    let mut executions = [0_usize; SEPARATION_SEQUENCE_ROWS];
    let mut first_trace_rows = [0_usize; SEPARATION_SEQUENCE_ROWS];
    let mut last_trace_rows = [0_usize; SEPARATION_SEQUENCE_ROWS];
    let mut status_masks = [0_u16; SEPARATION_SEQUENCE_ROWS];
    let mut iterations = [0_usize; SEPARATION_SEQUENCE_ROWS];
    let mut direct_drops = [0_usize; SEPARATION_SEQUENCE_ROWS];
    let mut singleton_drops = [0_usize; SEPARATION_SEQUENCE_ROWS];
    let mut first_generations = [0_usize; SEPARATION_SEQUENCE_ROWS];
    let mut last_generations = [0_usize; SEPARATION_SEQUENCE_ROWS];
    let mut certificates_passed = [true; SEPARATION_SEQUENCE_ROWS];
    for (trace_index, execution) in report.methods.iter().enumerate() {
        let method_index = separation_method_index(execution.method);
        if !requested[method_index] {
            return Err(invalid(
                "PPML separation sequence contains a method that was not requested",
            ));
        }
        validate_separation_execution(report, execution)?;
        executions[method_index] = checked_count_add(
            executions[method_index],
            1,
            "PPML separation sequence execution count",
        )?;
        let trace_row = trace_index
            .checked_add(1)
            .ok_or_else(|| allocation("PPML separation trace row overflows"))?;
        if first_trace_rows[method_index] == 0 {
            first_trace_rows[method_index] = trace_row;
            first_generations[method_index] = execution.generation;
        }
        last_trace_rows[method_index] = trace_row;
        last_generations[method_index] = execution.generation;
        let status = method_status_code(execution.status) as u32;
        status_masks[method_index] |= 1_u16 << (status - 1);
        iterations[method_index] = checked_count_add(
            iterations[method_index],
            execution.iterations,
            "PPML separation sequence iterations",
        )?;
        direct_drops[method_index] = checked_count_add(
            direct_drops[method_index],
            execution.dropped_observations.len(),
            "PPML separation sequence direct drops",
        )?;
        singleton_drops[method_index] = checked_count_add(
            singleton_drops[method_index],
            execution.singleton_drops.len(),
            "PPML separation sequence singleton drops",
        )?;
        certificates_passed[method_index] &= execution_certificate_passed(execution);
    }

    let mut output = reserved_filled(
        checked_product(
            SEPARATION_SEQUENCE_ROWS,
            SEPARATION_SEQUENCE_COLUMNS,
            "PPML separation-sequence evidence",
        )?,
        0.0,
        "PPML separation-sequence evidence",
    )?;
    for row in 0..SEPARATION_SEQUENCE_ROWS {
        let start = row * SEPARATION_SEQUENCE_COLUMNS;
        output[start] = exact_count(row + 1, "separation method code")?;
        output[start + 1] = f64::from(requested[row]);
        output[start + 2] = if row == 0 || !requested[row] {
            0.0
        } else {
            exact_count(row, "registered separation order")?
        };
        output[start + 3] = exact_count(executions[row], "separation executions")?;
        output[start + 4] = exact_count(first_trace_rows[row], "first separation trace row")?;
        output[start + 5] = exact_count(last_trace_rows[row], "last separation trace row")?;
        output[start + 6] = f64::from(status_masks[row]);
        output[start + 7] = exact_count(iterations[row], "separation iterations")?;
        output[start + 8] = exact_count(direct_drops[row], "separation direct drops")?;
        output[start + 9] = exact_count(singleton_drops[row], "separation singleton drops")?;
        output[start + 10] = exact_count(first_generations[row], "first separation generation")?;
        output[start + 11] = exact_count(last_generations[row], "last separation generation")?;
        output[start + 12] = f64::from(certificates_passed[row]);
    }
    Ok(output)
}

const fn separation_method_index(method: SeparationMethod) -> usize {
    match method {
        SeparationMethod::Singletons => 0,
        SeparationMethod::FixedEffects => 1,
        SeparationMethod::Simplex => 2,
        SeparationMethod::IteratedRectifier => 3,
        SeparationMethod::Mu => 4,
    }
}

struct StableDigest(u64);

impl StableDigest {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn word(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize, label: &'static str) -> Result<()> {
        self.word(
            u64::try_from(value)
                .map_err(|_| invalid(format!("PPML {label} does not fit its stable digest")))?,
        );
        Ok(())
    }

    fn framed_bytes(&mut self, bytes: &[u8], label: &'static str) -> Result<()> {
        self.usize(bytes.len(), label)?;
        self.bytes(bytes);
        Ok(())
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

fn separation_trace_digest(report: &SeparationReport) -> Result<u64> {
    let mut digest = StableDigest::new();
    digest.framed_bytes(b"fereg.ppml.separation-trace.v1.2", "trace digest domain")?;
    digest.usize(report.candidate_observations, "trace candidate count")?;
    digest.usize(report.singleton_count, "trace singleton count")?;
    digest.usize(report.separated_count, "trace separated count")?;
    digest.usize(report.generations, "trace generation count")?;
    digest.usize(report.methods.len(), "trace execution count")?;
    for (sequence, execution) in report.methods.iter().enumerate() {
        digest.usize(sequence, "trace sequence")?;
        digest.usize(execution.generation, "trace generation")?;
        digest.word(separation_method_code(execution.method) as u64);
        digest.word(method_status_code(execution.status) as u64);
        digest.usize(execution.iterations, "trace iterations")?;
        digest.usize(
            execution.dropped_observations.len(),
            "trace direct-drop count",
        )?;
        for row in &execution.dropped_observations {
            digest.usize(*row, "trace direct-drop row")?;
        }
        digest.usize(
            execution.singleton_drops.len(),
            "trace singleton-drop count",
        )?;
        for row in &execution.singleton_drops {
            digest.usize(*row, "trace singleton-drop row")?;
        }
        match &execution.certificate {
            None => digest.word(0),
            Some(certificate) => {
                digest.word(1);
                match certificate.proof {
                    SeparationCertificateProof::RecessionDirection => digest.word(1),
                    SeparationCertificateProof::SimplexFullPositiveRank { columns } => {
                        digest.word(2);
                        digest.usize(columns, "trace simplex positive rank")?;
                    }
                    SeparationCertificateProof::IteratedRectifierFixedPoint => digest.word(3),
                    SeparationCertificateProof::SimplexDualInfeasibility {
                        boundary_rows,
                        nullity,
                    } => {
                        digest.word(4);
                        digest.usize(boundary_rows, "trace simplex dual boundary rows")?;
                        digest.usize(nullity, "trace simplex dual nullity")?;
                    }
                }
                for value in [
                    certificate.interior_zero_relative,
                    certificate.boundary_violation_relative,
                    certificate.span_residual_relative,
                    certificate.negative_margin_relative,
                    certificate.direction_scale,
                    certificate.positive_maximum,
                ] {
                    digest.word(value.to_bits());
                }
                digest.word(u64::from(certificate.passed));
            }
        }
        match &execution.exact_fe_certificate {
            None => digest.word(0),
            Some(certificate) => {
                digest.word(1);
                digest.usize(
                    certificate.checked_intercept_effects,
                    "trace exact-FE checked effects",
                )?;
                digest.usize(
                    certificate.zero_only_levels.len(),
                    "trace exact-FE zero-level count",
                )?;
                for identity in &certificate.zero_only_levels {
                    digest.word(u64::from(identity.effect.get()));
                    digest.usize(
                        identity.representative_original_row,
                        "trace exact-FE representative original row",
                    )?;
                }
                digest.word(u64::from(certificate.passed));
            }
        }
        digest.usize(
            execution.workspace_observation_vectors,
            "trace workspace vectors",
        )?;
        digest.usize(
            execution.simplex_tableau_cells,
            "trace simplex tableau cells",
        )?;
        // Elapsed time and explanatory prose are deliberately excluded: they
        // are observational/process data, not deterministic ledger identity.
    }
    Ok(digest.finish())
}

fn generation_hash_digest(report: &SeparationReport) -> Result<u64> {
    let mut digest = StableDigest::new();
    digest.framed_bytes(
        b"fereg.ppml.generation-hashes.v1.1",
        "generation-hash digest domain",
    )?;
    digest.usize(
        report.generation_hashes.len(),
        "generation-hash sequence length",
    )?;
    for hash in &report.generation_hashes {
        digest.word(*hash);
    }
    Ok(digest.finish())
}

fn aggregate_separation(
    report: &SeparationReport,
    enabled: SeparationMethods,
) -> Result<(Vec<f64>, usize)> {
    let inventory = [
        (SeparationMethod::Singletons, true),
        (SeparationMethod::FixedEffects, enabled.fixed_effects),
        (SeparationMethod::Simplex, enabled.simplex),
        (
            SeparationMethod::IteratedRectifier,
            enabled.iterated_rectifier,
        ),
        (SeparationMethod::Mu, enabled.mu),
    ];
    let rows = inventory.iter().filter(|(_, enabled)| *enabled).count();
    for execution in &report.methods {
        let method_enabled = inventory
            .iter()
            .find_map(|(method, enabled)| (*method == execution.method).then_some(*enabled))
            .unwrap_or(false);
        if !method_enabled {
            return Err(invalid(
                "PPML separation report contains a disabled or unknown method",
            ));
        }
        validate_separation_execution(report, execution)?;
    }
    let mut output = reserved_filled(
        checked_product(rows, SEPARATION_COLUMNS, "PPML separation aggregate")?,
        0.0,
        "PPML separation aggregate",
    )?;
    let mut row = 0_usize;
    for (method, method_enabled) in inventory {
        if !method_enabled {
            continue;
        }
        let start = row * SEPARATION_COLUMNS;
        output[start] = separation_method_code(method);
        output[start + 1] = method_status_code(MethodStatus::NoCandidates);
        output[start + 7] = 1.0;
        for value in &mut output[start + 8..start + 12] {
            *value = STATA_SYSTEM_MISSING;
        }
        let mut execution_count = 0_usize;
        let mut latest_generation = 0_usize;
        let mut total_iterations = 0_usize;
        let mut direct_drops = 0_usize;
        let mut singleton_drops = 0_usize;
        let mut exact_checked = 0_usize;
        let mut exact_levels = 0_usize;
        let mut all_certificates_passed = true;
        let mut maximum_interior = None::<f64>;
        let mut maximum_boundary = None::<f64>;
        let mut maximum_span = None::<f64>;
        let mut minimum_margin = None::<f64>;
        for execution in report
            .methods
            .iter()
            .filter(|execution| execution.method == method)
        {
            execution_count = checked_count_add(execution_count, 1, "separation executions")?;
            latest_generation = latest_generation.max(execution.generation);
            total_iterations = checked_count_add(
                total_iterations,
                execution.iterations,
                "separation iterations",
            )?;
            direct_drops = checked_count_add(
                direct_drops,
                execution.dropped_observations.len(),
                "separation direct drops",
            )?;
            singleton_drops = checked_count_add(
                singleton_drops,
                execution.singleton_drops.len(),
                "separation singleton drops",
            )?;
            if method_status_code(execution.status) > output[start + 1] {
                output[start + 1] = method_status_code(execution.status);
            }
            all_certificates_passed &= execution_certificate_passed(execution);
            if let Some(certificate) = &execution.certificate {
                update_maximum(&mut maximum_interior, certificate.interior_zero_relative);
                update_maximum(
                    &mut maximum_boundary,
                    certificate.boundary_violation_relative,
                );
                update_maximum(&mut maximum_span, certificate.span_residual_relative);
                update_minimum(&mut minimum_margin, certificate.negative_margin_relative);
            }
            if let Some(certificate) = &execution.exact_fe_certificate {
                exact_checked = checked_count_add(
                    exact_checked,
                    certificate.checked_intercept_effects,
                    "exact-FE checked effects",
                )?;
                exact_levels = checked_count_add(
                    exact_levels,
                    certificate.zero_only_levels.len(),
                    "exact-FE zero-only levels",
                )?;
            }
        }
        output[start + 2] = exact_count(execution_count, "separation executions")?;
        output[start + 3] = exact_count(latest_generation, "separation generation")?;
        output[start + 4] = exact_count(total_iterations, "separation iterations")?;
        output[start + 5] = exact_count(direct_drops, "separation direct drops")?;
        output[start + 6] = exact_count(singleton_drops, "separation singleton drops")?;
        output[start + 7] = f64::from(all_certificates_passed);
        for (column, value) in [
            (8, maximum_interior),
            (9, maximum_boundary),
            (10, maximum_span),
            (11, minimum_margin),
        ] {
            if let Some(value) = value {
                output[start + column] = value;
            }
        }
        output[start + 12] = exact_count(exact_checked, "exact-FE checked effects")?;
        output[start + 13] = exact_count(exact_levels, "exact-FE zero-only levels")?;
        row += 1;
    }
    Ok((output, rows))
}

fn execution_certificate_passed(
    report: &fereg_core::ppml_separation::SeparationMethodReport,
) -> bool {
    match report.method {
        SeparationMethod::Singletons => true,
        SeparationMethod::FixedEffects if report.status == MethodStatus::NoCandidates => true,
        SeparationMethod::FixedEffects => report
            .exact_fe_certificate
            .as_ref()
            .is_some_and(|certificate| certificate.passed),
        SeparationMethod::Simplex | SeparationMethod::IteratedRectifier | SeparationMethod::Mu => {
            match report.status {
                MethodStatus::NoCandidates => true,
                MethodStatus::NoSeparation | MethodStatus::Certified => report
                    .certificate
                    .as_ref()
                    .is_some_and(|certificate| certificate.passed),
                MethodStatus::SkippedUnsupported
                | MethodStatus::SkippedMemory
                | MethodStatus::MaximumIterations
                | MethodStatus::Uncertified => false,
            }
        }
    }
}

fn update_maximum(target: &mut Option<f64>, value: f64) {
    debug_assert!(value.is_finite());
    *target = Some(target.map_or(value, |current| current.max(value)));
}

fn update_minimum(target: &mut Option<f64>, value: f64) {
    debug_assert!(value.is_finite());
    *target = Some(target.map_or(value, |current| current.min(value)));
}

fn validate_separation_execution(
    report: &SeparationReport,
    execution: &fereg_core::ppml_separation::SeparationMethodReport,
) -> Result<()> {
    let stable_rows = |rows: &[usize]| {
        rows.iter().enumerate().all(|(position, row)| {
            *row < report.candidate_observations && (position == 0 || rows[position - 1] < *row)
        })
    };
    if execution.generation > report.generations
        || !execution.elapsed_seconds.is_finite()
        || execution.elapsed_seconds < 0.0
        || !stable_rows(&execution.dropped_observations)
        || !stable_rows(&execution.singleton_drops)
        || execution
            .dropped_observations
            .iter()
            .any(|row| execution.singleton_drops.binary_search(row).is_ok())
    {
        return Err(invalid(
            "PPML separation execution has an invalid generation, timing, or row ledger",
        ));
    }
    if let Some(certificate) = &execution.certificate {
        let diagnostics = [
            certificate.interior_zero_relative,
            certificate.boundary_violation_relative,
            certificate.span_residual_relative,
            certificate.negative_margin_relative,
            certificate.direction_scale,
            certificate.positive_maximum,
        ];
        if diagnostics
            .iter()
            .any(|value| !is_stata_publishable(*value))
        {
            return Err(numerical_nonconvergence(
                "certified PPML separation diagnostic is not representable in Stata",
            ));
        }
        if diagnostics.iter().any(|value| *value < 0.0)
            || (certificate.direction_scale == 0.0
                && (execution.status != MethodStatus::NoSeparation
                    || diagnostics.iter().any(|value| *value != 0.0)))
        {
            return Err(invalid(
                "PPML separation certificate contains an invalid diagnostic",
            ));
        }
    }
    if let Some(certificate) = &execution.exact_fe_certificate {
        if certificate
            .zero_only_levels
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid(
                "PPML exact-FE separation certificate has duplicate or unordered levels",
            ));
        }
    }
    Ok(())
}

fn checked_count_add(left: usize, right: usize, label: &'static str) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| invalid(format!("PPML {label} overflowed")))
}

fn validate_dof_and_weight_identities(result: &CertifiedPpmlResult) -> Result<()> {
    let sample = result.sample();
    let weights = result.weights();
    let inference = result.inference();
    let dof = &inference.dof;
    let model_rank = dof
        .finite_within_rank
        .checked_add(dof.absorbed_rank)
        .ok_or_else(|| invalid("PPML model rank overflows"))?;
    let structural = dof
        .effective_observations
        .checked_sub(dof.model_rank)
        .ok_or_else(|| invalid("PPML model rank exceeds effective observations"))?;
    let cluster_minimum = result.clusters().iter().map(|cluster| cluster.groups).min();
    let expected_reported = cluster_minimum
        .map(|groups| structural.min(groups.saturating_sub(1)))
        .unwrap_or(structural);
    let weight_metadata_consistent = match weights.kind {
        None => {
            weights.raw_weight_sum.is_none()
                && weights.log_raw_weight_sum.is_none()
                && weights.raw_to_prepared_multiplier == Some(1.0)
                && weights.log_raw_to_prepared_multiplier == 0.0
        }
        Some(WeightKind::Frequency) => {
            weights.raw_weight_sum.is_some()
                && weights.log_raw_weight_sum.is_some()
                && weights.raw_to_prepared_multiplier == Some(1.0)
                && weights.log_raw_to_prepared_multiplier == 0.0
        }
        Some(WeightKind::Probability) => {
            weights.log_raw_weight_sum.is_some()
                && (weights.raw_to_prepared_multiplier.is_some()
                    || weights.log_raw_to_prepared_multiplier.abs() > f64::MAX.ln())
        }
        Some(WeightKind::Analytic | WeightKind::Importance) => false,
    };
    if weights.physical_observations != sample.original_rows().len()
        || weights.effective_observations != dof.effective_observations
        || dof.model_rank != model_rank
        || dof.structural_residual != structural
        || dof.reported != expected_reported
        || dof.minimum_cluster_groups != cluster_minimum
        || !weights.prepared_weight_sum.is_finite()
        || weights.prepared_weight_sum <= 0.0
        || weights
            .raw_weight_sum
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        || weights
            .raw_to_prepared_multiplier
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        || weights
            .log_raw_weight_sum
            .is_some_and(|value| !value.is_finite())
        || !weights.log_raw_to_prepared_multiplier.is_finite()
        || !weight_metadata_consistent
    {
        return Err(invalid(
            "PPML DoF, final-sample, cluster, or weight identities are inconsistent",
        ));
    }
    if !is_stata_publishable(weights.prepared_weight_sum)
        || weights
            .log_raw_weight_sum
            .is_some_and(|value| !is_stata_publishable(value))
        || !is_stata_publishable(weights.log_raw_to_prepared_multiplier)
    {
        return Err(numerical_nonconvergence(
            "certified PPML weight metadata is not representable in Stata",
        ));
    }
    let likelihood = result.likelihood();
    if !is_stata_publishable(likelihood.deviance)
        || !is_stata_publishable(likelihood.log_likelihood)
        || !is_stata_publishable(likelihood.null_mean)
        || !is_stata_publishable(likelihood.null_log_likelihood)
        || !is_stata_publishable(likelihood.pseudo_r_squared)
    {
        return Err(numerical_nonconvergence(
            "certified PPML likelihood statistics are not representable in Stata",
        ));
    }
    if likelihood.deviance < 0.0 || likelihood.null_mean <= 0.0 {
        return Err(invalid("PPML likelihood statistics are inconsistent"));
    }
    validate_inference_contract(result)
}

fn validate_inference_contract(result: &CertifiedPpmlResult) -> Result<()> {
    let inference = result.inference();
    let coefficients = result.coefficients();
    let wald = &inference.wald;
    if wald.degrees_of_freedom != wald.tested_columns.len()
        || wald.distribution != PpmlTestDistribution::ChiSquared
        || wald
            .tested_columns
            .iter()
            .enumerate()
            .any(|(position, slot)| {
                *slot >= coefficients.statuses().len()
                    || coefficients.statuses()[*slot] != PpmlCoefficientStatus::Identified
                    || (position > 0 && wald.tested_columns[position - 1] >= *slot)
            })
        || (wald.degrees_of_freedom == 0 && wald.statistic.is_some())
        || wald.statistic.is_some_and(|value| value < 0.0)
        || wald.solve_relative_infinity_norm < 0.0
    {
        return Err(invalid("PPML Wald report is inconsistent"));
    }
    if wald.reference.as_ref().is_some_and(|reference| {
        reference.slot() >= coefficients.statuses().len()
            || coefficients.statuses()[reference.slot()] != PpmlCoefficientStatus::Identified
            || wald.tested_columns.contains(&reference.slot())
            || wald.tested_columns.len() + 1 != coefficients.kept_full_slots().len()
            || coefficients
                .kept_full_slots()
                .iter()
                .any(|slot| *slot != reference.slot() && !wald.tested_columns.contains(slot))
            || reference.loadings().len() != wald.tested_columns.len()
            || reference
                .loadings()
                .iter()
                .any(|value| !is_stata_publishable(*value) || value.abs() > 1.0)
            || !is_stata_publishable(reference.span_error())
            || reference.span_error() < 0.0
            || !is_stata_publishable(reference.span_tolerance())
            || reference.span_tolerance() <= 0.0
            || reference.span_tolerance() >= 1.0
            || reference.span_error() > reference.span_tolerance()
    }) {
        return Err(invalid("PPML constant contrast report is inconsistent"));
    }
    if wald
        .statistic
        .is_some_and(|value| !is_stata_publishable(value))
        || !is_stata_publishable(wald.solve_relative_infinity_norm)
    {
        return Err(numerical_nonconvergence(
            "certified PPML Wald report is not representable in Stata",
        ));
    }
    let certificate = inference.vce.certificate;
    let norms = [
        certificate.bread_symmetry_relative_infinity_norm,
        certificate.meat_relative_infinity_norm,
        certificate.covariance_relative_infinity_norm,
        certificate.covariance_symmetry_relative_infinity_norm,
        certificate.wald_solve_relative_infinity_norm,
    ];
    if !is_stata_publishable(certificate.tolerance)
        || norms.iter().any(|value| !is_stata_publishable(*value))
    {
        return Err(numerical_nonconvergence(
            "certified PPML sandwich certificate is not representable in Stata",
        ));
    }
    if !certificate.passed
        || certificate.tolerance <= 0.0
        || certificate.tolerance >= 1.0
        || norms.iter().any(|value| *value < 0.0)
        || norms.iter().any(|value| *value > certificate.tolerance)
    {
        return Err(invalid("PPML sandwich certificate is inconsistent"));
    }
    let psd = inference.vce.psd;
    let psd_errors = [
        psd.eigensystem_reconstruction_relative_error,
        psd.orthogonality_relative_error,
        psd.clipped_reconstruction_relative_error,
    ];
    if !is_stata_publishable(psd.raw_minimum_eigenvalue)
        || !is_stata_publishable(psd.final_minimum_eigenvalue)
        || psd_errors.iter().any(|value| !is_stata_publishable(*value))
    {
        return Err(numerical_nonconvergence(
            "certified PPML PSD certificate is not representable in Stata",
        ));
    }
    if !psd.passed
        || psd.adjusted != (psd.clipped_eigenvalues > 0)
        || psd_errors.iter().any(|value| *value < 0.0)
        || psd_errors
            .iter()
            .any(|value| *value > certificate.tolerance)
        || match psd.state {
            PpmlPsdState::Strict | PpmlPsdState::MultiwayRaw => psd.adjusted,
            PpmlPsdState::MultiwayClipped => !psd.adjusted,
        }
    {
        return Err(invalid("PPML PSD certificate is inconsistent"));
    }
    Ok(())
}

fn weighted_outcome_sum(outcome: &[f64], weights: &[f64]) -> Result<f64> {
    if outcome.len() != weights.len() {
        return Err(invalid("PPML weighted-outcome inputs are misaligned"));
    }
    let mut sum = 0.0_f64;
    let mut compensation = 0.0_f64;
    for (&outcome, &weight) in outcome.iter().zip(weights) {
        let product = outcome * weight;
        if product < 0.0 {
            return Err(invalid("PPML weighted outcome is negative"));
        }
        if !is_stata_publishable(product) {
            return Err(numerical_nonconvergence(
                "PPML weighted outcome is not representable in Stata",
            ));
        }
        let next = sum + product;
        if sum.abs() >= product.abs() {
            compensation += (sum - next) + product;
        } else {
            compensation += (product - next) + sum;
        }
        sum = next;
    }
    let total = sum + compensation;
    if total < 0.0 {
        return Err(invalid("certified PPML weighted outcome sum is negative"));
    }
    if !is_stata_publishable(total) {
        return Err(numerical_nonconvergence(
            "certified PPML weighted outcome sum is not representable in Stata",
        ));
    }
    Ok(total)
}

fn validate_stata_cells(
    label: &str,
    values: &[f64],
    optional: &[usize],
    class: StataCellClass,
) -> Result<()> {
    if optional.windows(2).any(|pair| pair[0] >= pair[1])
        || optional.iter().any(|index| *index >= values.len())
    {
        return Err(invalid(format!(
            "PPML {label} optional-cell inventory is invalid",
        )));
    }
    validate_stata_cells_where(
        label,
        values,
        |index| optional.binary_search(&index).is_ok(),
        class,
    )
}

fn validate_stata_cells_where(
    label: &str,
    values: &[f64],
    optional: impl Fn(usize) -> bool,
    class: StataCellClass,
) -> Result<()> {
    for (index, value) in values.iter().copied().enumerate() {
        if value == STATA_SYSTEM_MISSING && optional(index) {
            continue;
        }
        if !is_stata_publishable(value) {
            let message = format!("PPML {label} shadow contains a nonpublishable value");
            return Err(match class {
                StataCellClass::Scientific => numerical_nonconvergence(message),
                StataCellClass::Structural => invalid(message),
            });
        }
    }
    Ok(())
}

fn relative_error(left: f64, right: f64) -> f64 {
    if !left.is_finite() || !right.is_finite() {
        f64::INFINITY
    } else {
        (left - right).abs() / left.abs().max(right.abs()).max(1.0)
    }
}

fn exact_count(value: usize, label: &'static str) -> Result<f64> {
    const MAX_EXACT_F64_INTEGER: u128 = 9_007_199_254_740_992;
    if value as u128 > MAX_EXACT_F64_INTEGER {
        Err(invalid(format!(
            "PPML {label} is not exactly representable"
        )))
    } else {
        Ok(value as f64)
    }
}

const fn ppml_status_code(status: PpmlCoefficientStatus) -> f64 {
    match status {
        PpmlCoefficientStatus::Identified => 1.0,
        PpmlCoefficientStatus::BaseLevel => 2.0,
        PpmlCoefficientStatus::SyntaxOmitted => 3.0,
        PpmlCoefficientStatus::CollinearWithFixedEffects => 4.0,
        PpmlCoefficientStatus::CollinearAfterResidualization => 5.0,
        PpmlCoefficientStatus::NumericallyRankDeficient => 6.0,
        PpmlCoefficientStatus::NormalizedStructuralConstant => 7.0,
    }
}

const fn weight_code(kind: Option<WeightKind>) -> f64 {
    match kind {
        None => 0.0,
        Some(WeightKind::Frequency) => 1.0,
        Some(WeightKind::Probability) => 2.0,
        Some(WeightKind::Analytic) => 3.0,
        Some(WeightKind::Importance) => 4.0,
    }
}

const fn memory_mode_code(mode: MemoryMode) -> f64 {
    match mode {
        MemoryMode::Auto => 0.0,
        MemoryMode::Fast => 1.0,
        MemoryMode::Low => 2.0,
    }
}

const fn memory_route_code(route: PpmlMemoryRoute) -> f64 {
    match route {
        PpmlMemoryRoute::Fast => 1.0,
        PpmlMemoryRoute::Low => 2.0,
    }
}

const fn linear_route_code(route: FeSolverKind) -> f64 {
    match route {
        FeSolverKind::Auto => 1.0,
        FeSolverKind::Map => 2.0,
        FeSolverKind::Lsmr => 3.0,
        FeSolverKind::Cg => 4.0,
        FeSolverKind::IronsTuck => 5.0,
        FeSolverKind::Squarem => 6.0,
        FeSolverKind::Cmg => 7.0,
    }
}

fn telemetry_u128(value: u128, label: &'static str) -> Result<f64> {
    const MAX_EXACT_F64_INTEGER: u128 = 9_007_199_254_740_992;
    if value > MAX_EXACT_F64_INTEGER {
        return Err(invalid(format!(
            "PPML {label} is not exactly representable",
        )));
    }
    Ok(value as f64)
}

fn telemetry_i128(value: i128, label: &'static str) -> Result<f64> {
    const MAX_EXACT_F64_INTEGER: i128 = 9_007_199_254_740_992;
    if !(-MAX_EXACT_F64_INTEGER..=MAX_EXACT_F64_INTEGER).contains(&value) {
        Err(invalid(format!(
            "PPML {label} is not exactly representable"
        )))
    } else {
        Ok(value as f64)
    }
}

const fn separation_method_code(method: SeparationMethod) -> f64 {
    match method {
        SeparationMethod::Singletons => 1.0,
        SeparationMethod::FixedEffects => 2.0,
        SeparationMethod::Simplex => 3.0,
        SeparationMethod::IteratedRectifier => 4.0,
        SeparationMethod::Mu => 5.0,
    }
}

const fn method_status_code(status: MethodStatus) -> f64 {
    match status {
        MethodStatus::NoCandidates => 1.0,
        MethodStatus::NoSeparation => 2.0,
        MethodStatus::Certified => 3.0,
        MethodStatus::SkippedUnsupported => 4.0,
        MethodStatus::SkippedMemory => 5.0,
        MethodStatus::MaximumIterations => 6.0,
        MethodStatus::Uncertified => 7.0,
    }
}

fn checked_product(left: usize, right: usize, label: &'static str) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| allocation(format!("{label} element count overflows")))
}

fn reserved_filled<T: Clone>(elements: usize, value: T, label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    output.resize(elements, value);
    Ok(output)
}

fn fallible_copy<T: Copy>(values: &[T], label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    output.extend_from_slice(values);
    Ok(output)
}

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Internal, message)
}

fn numerical_nonconvergence(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
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
}
