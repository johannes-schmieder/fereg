use std::collections::{BTreeMap, BTreeSet};

use crate::input::{PreparedMatrixStoreName, SpiApi};
use crate::result::{
    build_shadow_result, CertifiedOlsInference, OlsShadowResult, OlsVceKind, CERTIFICATE_ROWS,
    CLUSTER_COLUMNS, COEFFICIENT_STATUS_COLUMNS, DOF_ROWS, FE_COLUMNS, FIT_ROWS, RECEIPT_COLUMNS,
    RECEIPT_COMPLETE_COLUMN, SOLVER_COLUMNS, SOLVER_ROWS, SUBSET_COLUMNS,
};
use stfixest_core::error::{ErrorCode, Result, StFixestError};
use stfixest_core::metadata::{
    ModelWireV2, NumericWireTable, SourceId, ABSORB_COMPONENT_ROW_WIDTH, ABSORB_TERM_ROW_WIDTH,
    COMPONENT_ROW_WIDTH, SOURCE_ROW_WIDTH, TERM_ROW_WIDTH,
};
use stfixest_core::ols::{
    fit_prepared_ols_clustered_map, fit_prepared_ols_robust_map, fit_prepared_ols_unadjusted_map,
    prepare_ols_clustered_data, prepare_ols_data, LoadedNumericColumn, LoadedOlsInput,
    OlsClusterTermV1, OlsClusteredRequestV1, OlsInputBinding, OlsInputRole, OlsRequestV1,
    PreparedOlsAbsorbedDofPolicy, PreparedOlsClusteredConfig, PreparedOlsRobustConfig,
    PreparedOlsUnadjustedConfig,
};
use stfixest_core::protocol::{
    parse_key_values, Handshake, ACTION_PROTOCOL_VERSION, OLS_RESULT_WIRE_VERSION, PLUGIN_VERSION,
};

pub const ACTION_ESTIMATE_OLS_TEST: &str = "estimate-ols-test";
const CONFIRMATION: &str = "developer-estimation-test";
const BINDING_ROW_WIDTH: usize = 3;
const CLUSTER_TERM_ROW_WIDTH: usize = 2;
const CLUSTER_COMPONENT_ROW_WIDTH: usize = 3;

#[derive(Debug, Clone)]
struct ResultMatrices {
    b: String,
    covariance: String,
    coefficient_status: String,
    fit: String,
    dof: String,
    solver: String,
    fixed_effects: String,
    clusters: String,
    subsets: String,
    certificates: String,
    receipt: String,
}

#[derive(Debug)]
struct PreparedResultMatrices {
    b: PreparedMatrixStoreName,
    covariance: PreparedMatrixStoreName,
    coefficient_status: PreparedMatrixStoreName,
    fit: PreparedMatrixStoreName,
    dof: PreparedMatrixStoreName,
    solver: PreparedMatrixStoreName,
    fixed_effects: PreparedMatrixStoreName,
    clusters: PreparedMatrixStoreName,
    subsets: PreparedMatrixStoreName,
    certificates: PreparedMatrixStoreName,
    receipt: PreparedMatrixStoreName,
}

#[derive(Debug, Clone, Copy)]
struct ObservationOutputs {
    sample: u32,
    residual: Option<u32>,
    drop_reason: Option<u32>,
}

#[derive(Debug)]
struct EstimateAction {
    request: OlsRequestV1,
    clusters: Vec<OlsClusterTermV1>,
    vce: OlsVceKind,
    absorbed_dof: PreparedOlsAbsorbedDofPolicy,
    account_cluster_nesting: bool,
    outputs: ObservationOutputs,
    matrices: PreparedResultMatrices,
}

#[derive(Debug)]
struct EstimateActionSpec {
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
    keep_singletons: bool,
    vce: OlsVceKind,
    absorbed_dof: PreparedOlsAbsorbedDofPolicy,
    account_cluster_nesting: bool,
    outputs: ObservationOutputs,
    matrices: ResultMatrices,
}

pub fn estimate_ols_test(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {
    let specification = EstimateActionSpec::parse(args)?;
    let action = specification.decode_and_validate(spi)?;
    let loaded = load_ols_input(&action.request, spi)?;
    match action.vce {
        OlsVceKind::Iid => {
            let prepared = prepare_ols_data(&action.request, &loaded)?;
            let inference = fit_prepared_ols_unadjusted_map(
                &prepared,
                PreparedOlsUnadjustedConfig {
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsUnadjustedConfig::default()
                },
            )?;
            let shadow = build_shadow_result(
                &prepared,
                None,
                CertifiedOlsInference::Iid(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, spi)
        }
        OlsVceKind::Robust => {
            let prepared = prepare_ols_data(&action.request, &loaded)?;
            let inference = fit_prepared_ols_robust_map(
                &prepared,
                PreparedOlsRobustConfig {
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsRobustConfig::default()
                },
            )?;
            let shadow = build_shadow_result(
                &prepared,
                None,
                CertifiedOlsInference::Robust(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, spi)
        }
        OlsVceKind::Clustered => {
            let request = OlsClusteredRequestV1 {
                ols: action.request.clone(),
                clusters: action.clusters.clone(),
            };
            let prepared = prepare_ols_clustered_data(&request, &loaded)?;
            let inference = fit_prepared_ols_clustered_map(
                &prepared,
                PreparedOlsClusteredConfig {
                    absorbed_dof: action.absorbed_dof,
                    account_cluster_nesting: action.account_cluster_nesting,
                    ..PreparedOlsClusteredConfig::default()
                },
            )?;
            let shadow = build_shadow_result(
                prepared.ols(),
                Some(&prepared),
                CertifiedOlsInference::Clustered(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, spi)
        }
    }
}

impl EstimateActionSpec {
    fn parse(args: &[String]) -> Result<Self> {
        let values = parse_estimate_key_values(args)?;
        let handshake = Handshake::from_key_values(&values)?;
        handshake.validate()?;
        if handshake.protocol != ACTION_PROTOCOL_VERSION {
            return Err(protocol(format!(
                "{ACTION_ESTIMATE_OLS_TEST} requires action protocol {}.{}",
                ACTION_PROTOCOL_VERSION.major, ACTION_PROTOCOL_VERSION.minor,
            )));
        }
        if required(&values, "confirm")? != CONFIRMATION {
            return Err(unsupported(format!(
                "{ACTION_ESTIMATE_OLS_TEST} requires confirm={CONFIRMATION}",
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
            || result_minor > OLS_RESULT_WIRE_VERSION.minor
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
        let specification = Self {
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
            keep_singletons: parse_bool(&values, "keep_singletons")?,
            vce: parse_vce(required(&values, "vce")?)?,
            absorbed_dof: parse_dof(required(&values, "dof")?)?,
            account_cluster_nesting: parse_bool(&values, "account_cluster_nesting")?,
            outputs: ObservationOutputs {
                sample: parse_positive_u32(&values, "output_sample")?,
                residual: (output_residual != 0).then_some(output_residual),
                drop_reason: (output_drop_reason != 0).then_some(output_drop_reason),
            },
            matrices: ResultMatrices {
                b: required(&values, "output_b_matrix")?.to_owned(),
                covariance: required(&values, "output_v_matrix")?.to_owned(),
                coefficient_status: required(&values, "output_status_matrix")?.to_owned(),
                fit: required(&values, "output_fit_matrix")?.to_owned(),
                dof: required(&values, "output_dof_matrix")?.to_owned(),
                solver: required(&values, "output_solver_matrix")?.to_owned(),
                fixed_effects: required(&values, "output_fe_matrix")?.to_owned(),
                clusters: required(&values, "output_cluster_matrix")?.to_owned(),
                subsets: required(&values, "output_subset_matrix")?.to_owned(),
                certificates: required(&values, "output_certificate_matrix")?.to_owned(),
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
            "keep_singletons",
            "vce",
            "dof",
            "account_cluster_nesting",
            "output_sample",
            "output_residual",
            "output_drop_reason",
            "output_b_matrix",
            "output_v_matrix",
            "output_status_matrix",
            "output_fit_matrix",
            "output_dof_matrix",
            "output_solver_matrix",
            "output_fe_matrix",
            "output_cluster_matrix",
            "output_subset_matrix",
            "output_certificate_matrix",
            "output_receipt_matrix",
        ];
        for key in values.keys() {
            if !ALLOWED.contains(&key.as_str()) {
                return Err(syntax(format!(
                    "unknown {ACTION_ESTIMATE_OLS_TEST} field {key}",
                )));
            }
        }
        Ok(())
    }

    fn decode_and_validate(&self, spi: &mut dyn SpiApi) -> Result<EstimateAction> {
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
        let prepared_model = if self.vce == OlsVceKind::Clustered {
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
        let matrices = validate_layout(
            &request,
            self.outputs,
            &self.matrices,
            prepared_model.coefficients.len(),
            self.absorb_term_count.max(1),
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
        Ok(EstimateAction {
            request,
            clusters,
            vce: self.vce,
            absorbed_dof: self.absorbed_dof,
            account_cluster_nesting: self.account_cluster_nesting,
            outputs: self.outputs,
            matrices,
        })
    }
}

/// Stata 18 rejects a plugin call with the complete guarded estimate protocol
/// as "too many options" before native dispatch.  Keep the ordinary argv form
/// for the C and Fake SPI boundaries, and additionally accept one explicitly
/// tagged, byte-length-framed bundle for the hidden licensed-Stata caller.
/// Decoding is complete and canonical before the handshake and before any SPI
/// metadata or observation read.
fn parse_estimate_key_values(args: &[String]) -> Result<BTreeMap<String, String>> {
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

fn decode_estimate_bundle(bundle: &str) -> Result<Vec<String>> {
    const MAX_FIELDS: usize = 64;
    let bytes = bundle.as_bytes();
    let (count, mut cursor) = parse_bundle_decimal(bytes, 0, "field count")?;
    if count == 0 || count > MAX_FIELDS {
        return Err(syntax(
            "guarded OLS bundle field count must be between one and 64",
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

fn load_ols_input(request: &OlsRequestV1, spi: &mut dyn SpiApi) -> Result<LoadedOlsInput> {
    let n = spi.nobs()?;
    let mut candidate = reserved_filled(n, false, "OLS candidate marker")?;
    let candidate_position = role_position(request, InputRoleTag::Candidate)?;
    for observation in 1..=n {
        spi.check_interrupted()?;
        let (value, missing) = spi.read_numeric(candidate_position, observation)?;
        if missing || !value.is_finite() || !matches!(value, 0.0 | 1.0) {
            return Err(invalid_data(format!(
                "candidate marker at observation {observation} must be nonmissing zero or one",
            )));
        }
        candidate[observation - 1] = value == 1.0;
    }
    let mut outcome = empty_loaded_column(n, "OLS outcome")?;
    let mut numeric_sources = reserved_empty(request.model.sources.len(), "OLS source table")?;
    for source in &request.model.sources {
        push_reserved(
            &mut numeric_sources,
            (source.id, empty_loaded_column(n, "OLS numeric source")?),
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
    for (zero_based, retained_candidate) in candidate.iter().copied().enumerate() {
        if !retained_candidate {
            continue;
        }
        spi.check_interrupted()?;
        let observation = zero_based + 1;
        read_into(spi, outcome_position, observation, &mut outcome, zero_based)?;
        for ((_, destination), position) in numeric_sources.iter_mut().zip(&source_positions) {
            read_into(spi, *position, observation, destination, zero_based)?;
        }
        if let (Some(position), Some(destination)) = (analytic_position, analytic_weight.as_mut()) {
            read_into(spi, position, observation, destination, zero_based)?;
        }
        if let (Some(position), Some(destination)) = (frequency_position, frequency_weight.as_mut())
        {
            read_into(spi, position, observation, destination, zero_based)?;
        }
        if let (Some(position), Some(destination)) =
            (probability_position, probability_weight.as_mut())
        {
            read_into(spi, position, observation, destination, zero_based)?;
        }
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

fn precommit_and_store(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    validate_shadow_against_outputs(action, shadow)?;
    spi.check_interrupted()?;
    store_shadow(action, shadow, spi)
}

fn validate_shadow_against_outputs(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
) -> Result<()> {
    let matrix_dimension = shadow.coefficient_count.max(1);
    if shadow.b.len() != matrix_dimension
        || shadow.covariance.len() != matrix_dimension * matrix_dimension
        || shadow.coefficient_status.len() != matrix_dimension * COEFFICIENT_STATUS_COLUMNS
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
                (*reason == stfixest_core::DropReason::Retained).then_some(row)
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
    Ok(())
}

fn store_shadow(
    action: &EstimateAction,
    shadow: &OlsShadowResult<'_>,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    let mut retained_cursor = 0_usize;
    for (zero_based, reason) in shadow.drop_reasons.iter().copied().enumerate() {
        let observation = zero_based + 1;
        let retained = reason == stfixest_core::DropReason::Retained;
        spi.store_numeric(
            action.outputs.sample,
            observation,
            if retained { 1.0 } else { 0.0 },
        )?;
        if retained {
            debug_assert_eq!(shadow.retained_rows[retained_cursor], zero_based);
            if let Some(position) = action.outputs.residual {
                spi.store_numeric(position, observation, shadow.residuals[retained_cursor])?;
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
    outputs: ObservationOutputs,
    matrices: &ResultMatrices,
    coefficient_count: usize,
    fe_rows: usize,
    cluster_count: usize,
    spi: &mut dyn SpiApi,
    input_matrix_names: &[&String],
) -> Result<PreparedResultMatrices> {
    let nvars = spi.nvars()?;
    let output_count =
        1 + usize::from(outputs.residual.is_some()) + usize::from(outputs.drop_reason.is_some());
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
        validate_numeric_position(binding.stata_var_position, nvars, &mut positions, spi)?;
    }
    validate_numeric_position(outputs.sample, nvars, &mut positions, spi)?;
    if let Some(position) = outputs.residual {
        validate_numeric_position(position, nvars, &mut positions, spi)?;
    }
    if let Some(position) = outputs.drop_reason {
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

fn read_table(
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

fn read_bindings(spi: &mut dyn SpiApi, name: &str, count: usize) -> Result<Vec<OlsInputBinding>> {
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

fn decode_registry(value: &str, expected: usize) -> Result<Vec<String>> {
    if expected == 0 {
        if value != "." {
            return Err(invalid("empty registry must use the dot sentinel"));
        }
        return Ok(Vec::new());
    }
    let pieces: Vec<&str> = value.split('|').collect();
    if pieces.len() != expected {
        return Err(invalid(
            "registry cardinality does not match its declared count",
        ));
    }
    let mut output = reserved_empty(expected, "length-framed registry")?;
    for piece in pieces {
        let (length, payload) = piece
            .split_once(':')
            .ok_or_else(|| invalid("registry item lacks a length prefix"))?;
        let length = length
            .parse::<usize>()
            .map_err(|_| syntax("registry length must be an unsigned integer"))?;
        if payload.len() != length {
            return Err(invalid("registry byte length is incorrect"));
        }
        push_reserved(
            &mut output,
            payload.to_owned(),
            "registry capacity exhausted",
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
        _ => Err(unsupported(format!(
            "unsupported guarded OLS DoF policy {value:?}"
        ))),
    }
}

fn required<'a>(values: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str> {
    values
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| syntax(format!("missing required field {key}")))
}

fn parse_usize(values: &BTreeMap<String, String>, key: &str) -> Result<usize> {
    required(values, key)?
        .parse::<usize>()
        .map_err(|_| syntax(format!("field {key} must be an unsigned integer")))
}

fn parse_u16(values: &BTreeMap<String, String>, key: &str) -> Result<u16> {
    required(values, key)?
        .parse::<u16>()
        .map_err(|_| syntax(format!("field {key} must be a u16 integer")))
}

fn parse_positive_u32(values: &BTreeMap<String, String>, key: &str) -> Result<u32> {
    let value = parse_nonnegative_u32(values, key)?;
    if value == 0 {
        Err(syntax(format!("field {key} must be positive")))
    } else {
        Ok(value)
    }
}

fn parse_nonnegative_u32(values: &BTreeMap<String, String>, key: &str) -> Result<u32> {
    required(values, key)?
        .parse::<u32>()
        .map_err(|_| syntax(format!("field {key} must be a u32 integer")))
}

fn parse_bool(values: &BTreeMap<String, String>, key: &str) -> Result<bool> {
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

fn syntax(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Syntax, message)
}
fn invalid(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
}
fn invalid_data(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidData, message)
}
fn unsupported(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Unsupported, message)
}
fn protocol(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::ProtocolMismatch, message)
}
fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::RESULT_MAGIC;
    use std::collections::{BTreeMap, BTreeSet};
    use stfixest_core::metadata::{
        CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm,
        LevelState, MetadataFlag, ModelWireV2, RawTermKind, SourceEntry, TermComponent, TermId,
        TermOrder, TimeSeriesState,
    };
    use stfixest_core::protocol::{ACTION_PROTOCOL_VERSION, ADO_VERSION, MODEL_WIRE_VERSION};

    #[derive(Debug, Clone, PartialEq)]
    enum StoreEvent {
        Numeric(u32, usize, f64),
        Matrix(u8, usize, usize, f64),
    }

    #[derive(Clone)]
    struct FakeSpi {
        nobs: usize,
        nvars: usize,
        data: BTreeMap<(u32, usize), (f64, bool)>,
        matrices: BTreeMap<String, Vec<Vec<f64>>>,
        string_variables: BTreeSet<u32>,
        numeric_reads: usize,
        store_calls: usize,
        fail_store_at: Option<usize>,
        interrupt_at: Option<usize>,
        interrupt_checks: usize,
        stores: Vec<StoreEvent>,
        store_error: Option<StFixestError>,
    }

    impl SpiApi for FakeSpi {
        fn nobs(&mut self) -> Result<usize> {
            Ok(self.nobs)
        }

        fn nvars(&mut self) -> Result<usize> {
            Ok(self.nvars)
        }

        fn variable_is_string(&mut self, variable: u32) -> Result<bool> {
            Ok(self.string_variables.contains(&variable))
        }

        fn read_numeric(&mut self, variable: u32, observation: usize) -> Result<(f64, bool)> {
            self.numeric_reads += 1;
            self.data
                .get(&(variable, observation))
                .copied()
                .ok_or_else(|| invalid("test numeric cell is absent"))
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

        fn check_interrupted(&mut self) -> Result<()> {
            self.interrupt_checks += 1;
            if self.interrupt_at == Some(self.interrupt_checks) {
                Err(StFixestError::new(
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
            "RFIT" => 4,
            "RDOF" => 5,
            "RSOLVER" => 6,
            "RFE" => 7,
            "RCLUSTER" => 8,
            "RSUBSET" => 9,
            "RCERT" => 10,
            "RREC" => 11,
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
            format!("output_sample={sample_position}"),
            format!("output_residual={residual_position}"),
            format!("output_drop_reason={reason_position}"),
            "output_b_matrix=RB".to_owned(),
            "output_v_matrix=RV".to_owned(),
            "output_status_matrix=RSTATUS".to_owned(),
            "output_fit_matrix=RFIT".to_owned(),
            "output_dof_matrix=RDOF".to_owned(),
            "output_solver_matrix=RSOLVER".to_owned(),
            "output_fe_matrix=RFE".to_owned(),
            "output_cluster_matrix=RCLUSTER".to_owned(),
            "output_subset_matrix=RSUBSET".to_owned(),
            "output_certificate_matrix=RCERT".to_owned(),
            "output_receipt_matrix=RREC".to_owned(),
        ];
        (
            args,
            FakeSpi {
                nobs: 6,
                nvars: binding_count + 3,
                data,
                matrices: std::mem::take(&mut matrices),
                string_variables: BTreeSet::new(),
                numeric_reads: 0,
                store_calls: 0,
                fail_store_at: None,
                interrupt_at: None,
                interrupt_checks: 0,
                // The allocation-sensitive fake path must not grow this log
                // after the first output store.
                stores: Vec::with_capacity(512),
                store_error: Some(StFixestError::new(
                    ErrorCode::Internal,
                    "injected SPI output-store failure",
                )),
            },
        )
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
                Some(&StoreEvent::Matrix(11, 1, RECEIPT_COMPLETE_COLUMN, 1.0,)),
            );
        }
    }

    #[test]
    fn licensed_stata_bundle_matches_ordinary_argv_and_rejects_malformed_pre_read() {
        let (args, mut ordinary) = fixture(OlsVceKind::Iid);
        estimate_ols_test(&args, &mut ordinary).expect("ordinary argv succeeds");

        let (args, mut packed) = fixture(OlsVceKind::Iid);
        estimate_ols_test(&bundled(&args), &mut packed).expect("bundle succeeds");
        for name in [
            "RB", "RV", "RSTATUS", "RFIT", "RDOF", "RFE", "RCLUSTER", "RSUBSET", "RCERT", "RREC",
        ] {
            assert_eq!(packed.matrices[name], ordinary.matrices[name], "{name}");
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
        mixed.push("ado=0.1.0-phase01".to_owned());
        let error = estimate_ols_test(&mixed, &mut mixed_spi)
            .expect_err("mixed bundle and argv are rejected");
        assert_eq!(error.code, ErrorCode::Syntax);
        assert_eq!(mixed_spi.numeric_reads, 0);
        assert_eq!(mixed_spi.store_calls, 0);
    }

    #[test]
    fn every_output_store_failure_leaves_receipt_incomplete() {
        let (args, mut successful) = fixture(OlsVceKind::Iid);
        estimate_ols_test(&args, &mut successful).expect("baseline estimate succeeds");
        let total_stores = successful.store_calls;
        assert_eq!(total_stores, 216);
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
    fn omitted_factor_slot_is_zero_scattered_and_noconstant_layout_is_exact() {
        let (factor_args, mut factor_spi) = factor_scatter_fixture();
        estimate_ols_test(&factor_args, &mut factor_spi).expect("factor estimate succeeds");
        assert_eq!(factor_spi.matrices["RREC"][0][4], 3.0);
        assert_eq!(factor_spi.matrices["RB"][0][0], 0.0);
        assert_eq!(factor_spi.matrices["RSTATUS"][0][0], 1.0);
        assert_eq!(factor_spi.matrices["RSTATUS"][0][1], 2.0);
        assert_eq!(factor_spi.matrices["RSTATUS"][0][3], 0.0);
        for index in 0..3 {
            assert_eq!(factor_spi.matrices["RV"][0][index], 0.0);
            assert_eq!(factor_spi.matrices["RV"][index][0], 0.0);
        }
        assert!(matches!(factor_spi.matrices["RSTATUS"][1][1], 1.0 | 5.0));
        assert!(matches!(factor_spi.matrices["RSTATUS"][2][1], 1.0 | 5.0));

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
}
