use std::collections::BTreeSet;

use crate::output::OutputSink;
use stfixest_core::error::{ErrorCode, Result, StFixestError};
use stfixest_core::metadata::{
    ModelWireV2, NumericWireTable, SourceId, ABSORB_COMPONENT_ROW_WIDTH, ABSORB_TERM_ROW_WIDTH,
    COMPONENT_ROW_WIDTH, SOURCE_ROW_WIDTH, TERM_ROW_WIDTH,
};
use stfixest_core::ols::{
    prepare_ols_data, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
    OlsRequestV1, PreparedOlsData,
};
use stfixest_core::protocol::{parse_key_values, Handshake};

pub const ACTION_PREPARE_OLS_TEST: &str = "prepare-ols-test";
const CONFIRMATION: &str = "developer-preparation-test";
const BINDING_ROW_WIDTH: usize = 3;

/// Safe abstraction over a Stata-thread-confined SPI session.
pub trait SpiApi {
    fn nobs(&mut self) -> Result<usize>;
    fn nvars(&mut self) -> Result<usize>;
    fn variable_is_string(&mut self, variable: u32) -> Result<bool>;
    fn read_numeric(&mut self, variable: u32, observation: usize) -> Result<(f64, bool)>;
    fn matrix_dimensions(&mut self, name: &str) -> Result<(usize, usize)>;
    fn matrix_element(&mut self, name: &str, row: usize, column: usize) -> Result<f64>;
    fn store_numeric(&mut self, variable: u32, observation: usize, value: f64) -> Result<()>;
    fn check_interrupted(&mut self) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
struct DiagnosticBindings {
    original_observation: u32,
    retained: u32,
    drop_reason: u32,
}

#[derive(Debug)]
struct PrepareAction {
    request: OlsRequestV1,
    diagnostics: DiagnosticBindings,
}

/// Execute the developer-only numeric-main-effect/all-categorical-interaction/
/// optional-weight/simple-numeric-FE preparation action.
pub fn prepare_ols_test(
    args: &[String],
    output: &dyn OutputSink,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    let specification = PrepareActionSpec::parse(args)?;
    let action = specification.decode_and_validate(spi)?;
    let prepared = load_and_prepare(&action, spi)?;
    write_diagnostics(&action.diagnostics, &prepared, spi)?;
    output.display(&format!(
        concat!(
            "stfixest.prepare_ols_test=pass\n",
            "stfixest.prepare_ols.n_original={}\n",
            "stfixest.prepare_ols.n_candidate={}\n",
            "stfixest.prepare_ols.n_retained={}\n",
            "stfixest.prepare_ols.n_columns={}\n",
            "stfixest.prepare_ols.n_coefficient_slots={}\n",
            "stfixest.prepare_ols.n_categorical_sources={}\n",
            "stfixest.prepare_ols.n_absorb_terms={}\n",
            "stfixest.prepare_ols.n_fixed_effect_sources={}\n",
            "stfixest.prepare_ols.n_singletons={}\n",
            "stfixest.prepare_ols.keep_singletons={}\n"
        ),
        prepared.sample.n_original,
        prepared.sample.n_candidate,
        prepared.sample.n_active(),
        prepared.model.columns.len(),
        prepared.model.coefficients.len(),
        prepared.categorical_sources.len(),
        prepared.fixed_effects.len(),
        prepared
            .fixed_effects
            .iter()
            .map(|effect| effect.categorical_source_index)
            .collect::<BTreeSet<_>>()
            .len(),
        prepared.singletons.num_singletons,
        usize::from(prepared.singletons.kept_by_request),
    ));
    Ok(())
}

#[derive(Debug)]
struct PrepareActionSpec {
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
    binding_matrix: String,
    binding_count: usize,
    include_intercept: bool,
    keep_singletons: bool,
    diagnostics: DiagnosticBindings,
}

impl PrepareActionSpec {
    fn parse(args: &[String]) -> Result<Self> {
        let values = parse_key_values(args)?;
        Handshake::from_key_values(&values)?.validate()?;
        if required(&values, "confirm")? != CONFIRMATION {
            return Err(unsupported(format!(
                "{ACTION_PREPARE_OLS_TEST} requires confirm={CONFIRMATION}",
            )));
        }
        let source_count = parse_usize(&values, "source_count")?;
        let term_count = parse_usize(&values, "term_count")?;
        let component_count = parse_usize(&values, "component_count")?;
        let absorb_term_count = parse_usize(&values, "absorb_term_count")?;
        let absorb_component_count = parse_usize(&values, "absorb_component_count")?;
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
            binding_matrix: required(&values, "binding_matrix")?.to_owned(),
            binding_count: parse_usize(&values, "binding_count")?,
            include_intercept: parse_bool(&values, "include_intercept")?,
            keep_singletons: parse_bool(&values, "keep_singletons")?,
            diagnostics: DiagnosticBindings {
                original_observation: parse_positive_u32(&values, "output_original_observation")?,
                retained: parse_positive_u32(&values, "output_retained")?,
                drop_reason: parse_positive_u32(&values, "output_drop_reason")?,
            },
        };
        specification.reject_unknown_fields(&values)?;
        Ok(specification)
    }

    fn reject_unknown_fields(
        &self,
        values: &std::collections::BTreeMap<String, String>,
    ) -> Result<()> {
        const ALLOWED: [&str; 30] = [
            "ado",
            "protocol_major",
            "protocol_minor",
            "stata",
            "confirm",
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
            "binding_matrix",
            "binding_count",
            "include_intercept",
            "keep_singletons",
            "output_original_observation",
            "output_retained",
            "output_drop_reason",
        ];
        for key in values.keys() {
            if !ALLOWED.contains(&key.as_str()) {
                return Err(syntax(format!(
                    "unknown {ACTION_PREPARE_OLS_TEST} field {key}",
                )));
            }
        }
        Ok(())
    }

    fn decode_and_validate(&self, spi: &mut dyn SpiApi) -> Result<PrepareAction> {
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
        let specification = wire.decode_specification()?;
        let bindings = read_bindings(spi, &self.binding_matrix, self.binding_count)?;
        let request = OlsRequestV1 {
            model: specification.model,
            absorbed_terms: specification.absorbed_terms,
            bindings,
            include_intercept: self.include_intercept,
            keep_singletons: self.keep_singletons,
        };
        request.validate_and_compile()?;
        validate_spi_layout(&request, self.diagnostics, spi)?;
        Ok(PrepareAction {
            request,
            diagnostics: self.diagnostics,
        })
    }
}

fn load_and_prepare(action: &PrepareAction, spi: &mut dyn SpiApi) -> Result<PreparedOlsData> {
    let n = spi.nobs()?;
    let mut candidate = reserved_filled(n, false, "candidate marker")?;

    let candidate_position = role_position(&action.request, OlsInputRoleTag::Candidate)?;
    for observation in 1..=n {
        spi.check_interrupted()?;
        let (value, missing) = spi.read_numeric(candidate_position, observation)?;
        if missing || !value.is_finite() || !matches!(value, 0.0 | 1.0) {
            return Err(invalid_data(format!(
                "candidate marker at observation {observation} must be nonmissing and exactly zero or one",
            )));
        }
        candidate[observation - 1] = value == 1.0;
    }

    let mut outcome = empty_loaded_column(n, "outcome")?;
    let mut numeric_sources = Vec::new();
    numeric_sources
        .try_reserve_exact(action.request.model.sources.len())
        .map_err(|_| allocation("unable to reserve numeric source table"))?;
    for source in &action.request.model.sources {
        numeric_sources.push((source.id, empty_loaded_column(n, "numeric source")?));
    }

    let outcome_position = role_position(&action.request, OlsInputRoleTag::Outcome)?;
    let analytic_weight_position = action.request.bindings.iter().find_map(|binding| {
        (binding.role == OlsInputRole::AnalyticWeight).then_some(binding.stata_var_position)
    });
    let frequency_weight_position = action.request.bindings.iter().find_map(|binding| {
        (binding.role == OlsInputRole::FrequencyWeight).then_some(binding.stata_var_position)
    });
    let probability_weight_position = action.request.bindings.iter().find_map(|binding| {
        (binding.role == OlsInputRole::ProbabilityWeight).then_some(binding.stata_var_position)
    });
    let mut analytic_weight = analytic_weight_position
        .map(|_| empty_loaded_column(n, "analytic weight"))
        .transpose()?;
    let mut frequency_weight = frequency_weight_position
        .map(|_| empty_loaded_column(n, "frequency weight"))
        .transpose()?;
    let mut probability_weight = probability_weight_position
        .map(|_| empty_loaded_column(n, "probability weight"))
        .transpose()?;
    let source_positions = action
        .request
        .model
        .sources
        .iter()
        .map(|source| source_position(&action.request, source.id))
        .collect::<Result<Vec<_>>>()?;

    for (zero_based, retained_candidate) in candidate.iter().copied().enumerate() {
        if !retained_candidate {
            continue;
        }
        spi.check_interrupted()?;
        let observation = zero_based + 1;
        let (value, missing) = spi.read_numeric(outcome_position, observation)?;
        outcome.values[zero_based] = value;
        outcome.missing[zero_based] = missing;
        for ((_, destination), position) in numeric_sources.iter_mut().zip(&source_positions) {
            let (value, missing) = spi.read_numeric(*position, observation)?;
            destination.values[zero_based] = value;
            destination.missing[zero_based] = missing;
        }
        if let (Some(position), Some(destination)) =
            (analytic_weight_position, analytic_weight.as_mut())
        {
            let (value, missing) = spi.read_numeric(position, observation)?;
            destination.values[zero_based] = value;
            destination.missing[zero_based] = missing;
        }
        if let (Some(position), Some(destination)) =
            (frequency_weight_position, frequency_weight.as_mut())
        {
            let (value, missing) = spi.read_numeric(position, observation)?;
            destination.values[zero_based] = value;
            destination.missing[zero_based] = missing;
        }
        if let (Some(position), Some(destination)) =
            (probability_weight_position, probability_weight.as_mut())
        {
            let (value, missing) = spi.read_numeric(position, observation)?;
            destination.values[zero_based] = value;
            destination.missing[zero_based] = missing;
        }
    }

    prepare_ols_data(
        &action.request,
        &LoadedOlsInput {
            candidate,
            outcome,
            numeric_sources,
            analytic_weight,
            frequency_weight,
            probability_weight,
        },
    )
}

fn write_diagnostics(
    bindings: &DiagnosticBindings,
    prepared: &PreparedOlsData,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    // The last interruption check occurs before the first diagnostic mutation.
    // All decoding, validation, loading, and native preparation has succeeded.
    spi.check_interrupted()?;
    for (zero_based, reason) in prepared.sample.drop_reasons.iter().copied().enumerate() {
        let observation = zero_based + 1;
        spi.store_numeric(
            bindings.original_observation,
            observation,
            observation as f64,
        )?;
        spi.store_numeric(
            bindings.retained,
            observation,
            if reason == stfixest_core::DropReason::Retained {
                1.0
            } else {
                0.0
            },
        )?;
        spi.store_numeric(bindings.drop_reason, observation, f64::from(reason.code()))?;
    }
    Ok(())
}

fn validate_spi_layout(
    request: &OlsRequestV1,
    diagnostics: DiagnosticBindings,
    spi: &mut dyn SpiApi,
) -> Result<()> {
    let nvars = spi.nvars()?;
    let expected = request
        .bindings
        .len()
        .checked_add(3)
        .ok_or_else(|| invalid("plugin varlist width overflows"))?;
    if nvars != expected {
        return Err(invalid(format!(
            "plugin varlist has {nvars} variables, expected exactly {expected}",
        )));
    }
    let mut positions = BTreeSet::new();
    for binding in &request.bindings {
        validate_numeric_position(binding.stata_var_position, nvars, &mut positions, spi)?;
    }
    for position in [
        diagnostics.original_observation,
        diagnostics.retained,
        diagnostics.drop_reason,
    ] {
        validate_numeric_position(position, nvars, &mut positions, spi)?;
    }
    let nvars_u32 = u32::try_from(nvars)
        .map_err(|_| invalid("plugin varlist width exceeds the binding protocol"))?;
    if positions.len() != nvars || positions.iter().copied().ne(1..=nvars_u32) {
        return Err(invalid(
            "input and diagnostic bindings must cover the plugin varlist exactly",
        ));
    }
    Ok(())
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
        return Err(invalid(
            "plugin input and diagnostic positions must be unique and in range",
        ));
    }
    if spi.variable_is_string(position)? {
        return Err(unsupported(format!(
            "plugin varlist position {position} must be numeric",
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
    if rows != expected_rows || columns != width {
        return Err(invalid(format!(
            "{label} matrix {name:?} is {rows}x{columns}, expected {expected_rows}x{width}",
        )));
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(rows)
        .map_err(|_| allocation(format!("unable to reserve {label} matrix rows")))?;
    for row in 1..=rows {
        let mut output_row = Vec::new();
        output_row
            .try_reserve_exact(width)
            .map_err(|_| allocation(format!("unable to reserve {label} matrix row")))?;
        for column in 1..=width {
            output_row.push(spi.matrix_element(name, row, column)?);
        }
        values.push(output_row);
    }
    Ok(NumericWireTable {
        declared_count: declared_count as f64,
        rows: values,
    })
}

fn read_bindings(spi: &mut dyn SpiApi, name: &str, count: usize) -> Result<Vec<OlsInputBinding>> {
    if count == 0 {
        return Err(invalid(
            "binding matrix must contain candidate and outcome rows",
        ));
    }
    let (rows, columns) = spi.matrix_dimensions(name)?;
    if rows != count || columns != BINDING_ROW_WIDTH {
        return Err(invalid(format!(
            "binding matrix {name:?} is {rows}x{columns}, expected {count}x{BINDING_ROW_WIDTH}",
        )));
    }
    let mut bindings = Vec::new();
    bindings
        .try_reserve_exact(count)
        .map_err(|_| allocation("unable to reserve OLS input bindings"))?;
    for row in 1..=rows {
        let position = exact_positive_u32(spi.matrix_element(name, row, 1)?, "binding position")?;
        let role = exact_nonnegative_u32(spi.matrix_element(name, row, 2)?, "binding role")?;
        let source = exact_nonnegative_u32(spi.matrix_element(name, row, 3)?, "binding source")?;
        let role = match (role, source) {
            (1, 0) => OlsInputRole::Candidate,
            (2, 0) => OlsInputRole::Outcome,
            (3, source) if source > 0 => OlsInputRole::NumericSource(SourceId::new(source)?),
            (4, 0) => OlsInputRole::AnalyticWeight,
            (5, 0) => OlsInputRole::FrequencyWeight,
            (6, 0) => OlsInputRole::ProbabilityWeight,
            _ => return Err(invalid(format!("invalid OLS input binding at row {row}"))),
        };
        bindings.push(OlsInputBinding {
            stata_var_position: position,
            role,
        });
    }
    Ok(bindings)
}

#[derive(Clone, Copy)]
enum OlsInputRoleTag {
    Candidate,
    Outcome,
}

fn role_position(request: &OlsRequestV1, target: OlsInputRoleTag) -> Result<u32> {
    request
        .bindings
        .iter()
        .find_map(|binding| match (target, binding.role) {
            (OlsInputRoleTag::Candidate, OlsInputRole::Candidate)
            | (OlsInputRoleTag::Outcome, OlsInputRole::Outcome) => Some(binding.stata_var_position),
            _ => None,
        })
        .ok_or_else(|| invalid("validated OLS role binding disappeared"))
}

fn source_position(request: &OlsRequestV1, source: SourceId) -> Result<u32> {
    request
        .bindings
        .iter()
        .find_map(|binding| match binding.role {
            OlsInputRole::NumericSource(candidate) if candidate == source => {
                Some(binding.stata_var_position)
            }
            _ => None,
        })
        .ok_or_else(|| invalid("validated OLS source binding disappeared"))
}

fn empty_loaded_column(n: usize, label: &str) -> Result<LoadedNumericColumn> {
    Ok(LoadedNumericColumn {
        values: reserved_filled(n, 0.0, &format!("{label} values"))?,
        missing: reserved_filled(n, false, &format!("{label} missing mask"))?,
    })
}

fn reserved_filled<T: Clone>(n: usize, value: T, label: &str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(n)
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    output.resize(n, value);
    Ok(output)
}

fn decode_registry(encoded: &str, expected: usize) -> Result<Vec<String>> {
    if expected == 0 {
        return if encoded == "." {
            Ok(Vec::new())
        } else {
            Err(invalid("empty registry must use the '.' sentinel"))
        };
    }
    let bytes = encoded.as_bytes();
    let mut cursor = 0_usize;
    let mut output = Vec::new();
    output
        .try_reserve_exact(expected)
        .map_err(|_| allocation("unable to reserve model string registry"))?;
    while cursor < bytes.len() {
        let colon = bytes[cursor..]
            .iter()
            .position(|byte| *byte == b':')
            .map(|offset| cursor + offset)
            .ok_or_else(|| invalid("registry entry is missing its length separator"))?;
        let length_text = std::str::from_utf8(&bytes[cursor..colon])
            .map_err(|_| invalid("registry length is not UTF-8"))?;
        let length = length_text
            .parse::<usize>()
            .map_err(|_| invalid("registry length is not a nonnegative integer"))?;
        let start = colon + 1;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| invalid("registry entry length exceeds its encoded value"))?;
        output.push(
            std::str::from_utf8(&bytes[start..end])
                .map_err(|_| invalid("registry entry is not UTF-8"))?
                .to_owned(),
        );
        cursor = end;
        if cursor < bytes.len() {
            if bytes[cursor] != b'|' {
                return Err(invalid("registry entries must be separated by '|'"));
            }
            cursor += 1;
            if cursor == bytes.len() {
                return Err(invalid("registry must not end with an empty separator"));
            }
        }
    }
    if output.len() != expected {
        return Err(invalid(format!(
            "registry has {} entries, expected {expected}",
            output.len(),
        )));
    }
    Ok(output)
}

fn required<'a>(
    values: &'a std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str> {
    values.get(key).map(String::as_str).ok_or_else(|| {
        syntax(format!(
            "missing required {ACTION_PREPARE_OLS_TEST} field {key}=...",
        ))
    })
}

fn parse_u16(values: &std::collections::BTreeMap<String, String>, key: &str) -> Result<u16> {
    required(values, key)?
        .parse::<u16>()
        .map_err(|_| syntax(format!("field {key} must be an unsigned integer")))
}

fn parse_usize(values: &std::collections::BTreeMap<String, String>, key: &str) -> Result<usize> {
    required(values, key)?
        .parse::<usize>()
        .map_err(|_| syntax(format!("field {key} must be a nonnegative integer")))
}

fn parse_positive_u32(
    values: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<u32> {
    let value = required(values, key)?
        .parse::<u32>()
        .map_err(|_| syntax(format!("field {key} must be a positive integer")))?;
    if value == 0 {
        Err(syntax(format!("field {key} must be positive")))
    } else {
        Ok(value)
    }
}

fn parse_bool(values: &std::collections::BTreeMap<String, String>, key: &str) -> Result<bool> {
    match required(values, key)? {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(syntax(format!("field {key} must be exactly zero or one"))),
    }
}

fn exact_positive_u32(value: f64, label: &str) -> Result<u32> {
    let parsed = exact_nonnegative_u32(value, label)?;
    if parsed == 0 {
        Err(invalid(format!("{label} must be positive")))
    } else {
        Ok(parsed)
    }
}

fn exact_nonnegative_u32(value: f64, label: &str) -> Result<u32> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(invalid(format!(
            "{label} must be a nonnegative integer representable as u32",
        )));
    }
    Ok(value as u32)
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

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::BufferSink;
    use std::collections::{BTreeMap, BTreeSet};
    use stfixest_core::metadata::{
        AbsorbComponentRole, AbsorbTermId, AbsorbedTerm, CoefficientSlot,
        CompiledModelSpecification, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm,
        LevelState, MetadataFlag, ModelWireV2, RawTermKind, SourceEntry, TermComponent, TermId,
        TermOrder, TimeSeriesState,
    };
    use stfixest_core::protocol::{ACTION_PROTOCOL_VERSION, ADO_VERSION, MODEL_WIRE_VERSION};

    #[derive(Debug)]
    struct FakeSpi {
        nobs: usize,
        nvars: usize,
        strings: BTreeSet<u32>,
        data: BTreeMap<(u32, usize), (f64, bool)>,
        matrices: BTreeMap<String, Vec<Vec<f64>>>,
        reads: Vec<(u32, usize)>,
        writes: Vec<(u32, usize, f64)>,
        fail_read: Option<(u32, usize)>,
        interrupt_at_check: Option<usize>,
        interruption_checks: usize,
    }

    impl SpiApi for FakeSpi {
        fn nobs(&mut self) -> Result<usize> {
            Ok(self.nobs)
        }

        fn nvars(&mut self) -> Result<usize> {
            Ok(self.nvars)
        }

        fn variable_is_string(&mut self, variable: u32) -> Result<bool> {
            Ok(self.strings.contains(&variable))
        }

        fn read_numeric(&mut self, variable: u32, observation: usize) -> Result<(f64, bool)> {
            self.reads.push((variable, observation));
            if self.fail_read == Some((variable, observation)) {
                return Err(StFixestError::new(
                    ErrorCode::Internal,
                    "injected SPI read failure",
                ));
            }
            self.data
                .get(&(variable, observation))
                .copied()
                .ok_or_else(|| invalid("fake SPI data coordinate is absent"))
        }

        fn matrix_dimensions(&mut self, name: &str) -> Result<(usize, usize)> {
            let matrix = self
                .matrices
                .get(name)
                .ok_or_else(|| invalid("fake SPI matrix is absent"))?;
            Ok((matrix.len(), matrix.first().map_or(0, Vec::len)))
        }

        fn matrix_element(&mut self, name: &str, row: usize, column: usize) -> Result<f64> {
            self.matrices
                .get(name)
                .and_then(|matrix| matrix.get(row - 1))
                .and_then(|values| values.get(column - 1))
                .copied()
                .ok_or_else(|| invalid("fake SPI matrix coordinate is absent"))
        }

        fn store_numeric(&mut self, variable: u32, observation: usize, value: f64) -> Result<()> {
            self.writes.push((variable, observation, value));
            Ok(())
        }

        fn check_interrupted(&mut self) -> Result<()> {
            self.interruption_checks += 1;
            if self.interrupt_at_check == Some(self.interruption_checks) {
                Err(StFixestError::new(
                    ErrorCode::Interrupted,
                    "injected interruption",
                ))
            } else {
                Ok(())
            }
        }
    }

    fn fixture() -> (Vec<String>, FakeSpi) {
        let source = SourceEntry {
            id: SourceId::new(1).expect("source"),
            canonical_name: "x".to_owned(),
        };
        let graph = CompiledTermGraph {
            sources: vec![source.clone()],
            terms: vec![ExpandedTerm {
                id: TermId::new(1).expect("term"),
                order: TermOrder::new(1).expect("order"),
                coefficient_slot: CoefficientSlot::new(1).expect("slot"),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: "c.x".to_owned(),
                components: vec![TermComponent {
                    order: ComponentOrder::new(1).expect("component"),
                    source: source.id,
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
        let matrices = BTreeMap::from([
            ("S".to_owned(), wire.sources.rows),
            ("T".to_owned(), wire.terms.rows),
            ("C".to_owned(), wire.components.rows),
            ("A".to_owned(), wire.absorb_terms.rows),
            ("AC".to_owned(), wire.absorb_components.rows),
            (
                "B".to_owned(),
                vec![
                    vec![1.0, 1.0, 0.0],
                    vec![2.0, 2.0, 0.0],
                    vec![3.0, 3.0, 1.0],
                ],
            ),
        ]);
        let mut data = BTreeMap::new();
        let candidates = [0.0, 1.0, 1.0, 0.0, 1.0];
        for (index, value) in candidates.into_iter().enumerate() {
            data.insert((1, index + 1), (value, false));
        }
        for (observation, y, x, x_missing) in [
            (2, -0.0, 2.0, false),
            (3, 3.0, 9.0, true),
            (5, 5.0, f64::from_bits(1), false),
        ] {
            data.insert((2, observation), (y, false));
            data.insert((3, observation), (x, x_missing));
        }
        let spi = FakeSpi {
            nobs: 5,
            nvars: 6,
            strings: BTreeSet::new(),
            data,
            matrices,
            reads: Vec::new(),
            writes: Vec::new(),
            fail_read: None,
            interrupt_at_check: None,
            interruption_checks: 0,
        };
        let args = vec![
            format!("ado={ADO_VERSION}"),
            format!("protocol_major={}", ACTION_PROTOCOL_VERSION.major),
            format!("protocol_minor={}", ACTION_PROTOCOL_VERSION.minor),
            "stata=18.0".to_owned(),
            format!("confirm={CONFIRMATION}"),
            format!("model_major={}", MODEL_WIRE_VERSION.major),
            format!("model_minor={}", MODEL_WIRE_VERSION.minor),
            "source_matrix=S".to_owned(),
            "source_count=1".to_owned(),
            "term_matrix=T".to_owned(),
            "term_count=1".to_owned(),
            "component_matrix=C".to_owned(),
            "component_count=1".to_owned(),
            "source_names=1:x".to_owned(),
            "canonical_tokens=3:c.x".to_owned(),
            "raw_component_operators=1:c".to_owned(),
            "raw_time_series_operators=0:".to_owned(),
            "absorb_term_matrix=A".to_owned(),
            "absorb_term_count=0".to_owned(),
            "absorb_component_matrix=AC".to_owned(),
            "absorb_component_count=0".to_owned(),
            "absorb_labels=.".to_owned(),
            "absorb_saved_names=.".to_owned(),
            "binding_matrix=B".to_owned(),
            "binding_count=3".to_owned(),
            "include_intercept=1".to_owned(),
            "keep_singletons=0".to_owned(),
            "output_original_observation=4".to_owned(),
            "output_retained=5".to_owned(),
            "output_drop_reason=6".to_owned(),
        ];
        (args, spi)
    }

    fn analytic_weight_fixture() -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = fixture();
        spi.nvars = 7;
        spi.matrices
            .get_mut("B")
            .expect("bindings")
            .push(vec![4.0, 4.0, 0.0]);
        for value in &mut args {
            if value == "binding_count=3" {
                *value = "binding_count=4".to_owned();
            } else if value == "output_original_observation=4" {
                *value = "output_original_observation=5".to_owned();
            } else if value == "output_retained=5" {
                *value = "output_retained=6".to_owned();
            } else if value == "output_drop_reason=6" {
                *value = "output_drop_reason=7".to_owned();
            }
        }
        spi.data.insert((3, 3), (9.0, false));
        spi.data.insert((4, 2), (2.0, false));
        spi.data.insert((4, 3), (0.0, false));
        spi.data.insert((4, 5), (7.0, true));
        (args, spi)
    }

    fn frequency_weight_fixture() -> (Vec<String>, FakeSpi) {
        let (args, mut spi) = analytic_weight_fixture();
        spi.matrices.get_mut("B").expect("bindings")[3][1] = 5.0;
        spi.data.insert((4, 2), (2.0, false));
        spi.data.insert((4, 3), (3.0, false));
        spi.data.insert((4, 5), (4.0, false));
        (args, spi)
    }

    fn probability_weight_fixture() -> (Vec<String>, FakeSpi) {
        let (args, mut spi) = analytic_weight_fixture();
        spi.matrices.get_mut("B").expect("bindings")[3][1] = 6.0;
        spi.data.insert((4, 2), (0.8, false));
        spi.data.insert((4, 3), (1.1, false));
        spi.data.insert((4, 5), (1.4, false));
        (args, spi)
    }

    fn factor_fixture() -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = fixture();
        let source = SourceEntry {
            id: SourceId::new(1).expect("source"),
            canonical_name: "g".to_owned(),
        };
        let terms = [
            (1, 1.0, true, true, "1b.g", "1b"),
            (2, 2.0, false, false, "2.g", "2"),
            (3, 3.0, false, false, "3.g", "3"),
        ]
        .into_iter()
        .map(
            |(index, level, base, omitted, token, operator)| ExpandedTerm {
                id: TermId::new(index).expect("term"),
                order: TermOrder::new(index).expect("order"),
                coefficient_slot: CoefficientSlot::new(index).expect("slot"),
                raw_kind: RawTermKind::Factor,
                whole_column_omitted: omitted,
                canonical_token: token.to_owned(),
                components: vec![TermComponent {
                    order: ComponentOrder::new(1).expect("component"),
                    source: source.id,
                    kind: ComponentKind::Categorical,
                    level_state: LevelState::Present,
                    level_value: level,
                    base: if base {
                        MetadataFlag::True
                    } else {
                        MetadataFlag::False
                    },
                    omitted: if omitted {
                        MetadataFlag::True
                    } else {
                        MetadataFlag::False
                    },
                    raw_operator: operator.to_owned(),
                    time_series_state: TimeSeriesState::None,
                    raw_time_series_operator: String::new(),
                }],
            },
        )
        .collect::<Vec<_>>();
        let wire = ModelWireV2::from_graph(&CompiledTermGraph {
            sources: vec![source],
            terms,
        })
        .expect("factor wire");
        spi.matrices.insert("S".to_owned(), wire.sources.rows);
        spi.matrices.insert("T".to_owned(), wire.terms.rows);
        spi.matrices.insert("C".to_owned(), wire.components.rows);
        spi.data.insert((3, 2), (1.0, false));
        spi.data.insert((3, 3), (2.0, true));
        spi.data.insert((3, 5), (3.0, false));
        for value in &mut args {
            match value.as_str() {
                "term_count=1" => *value = "term_count=3".to_owned(),
                "component_count=1" => *value = "component_count=3".to_owned(),
                "source_names=1:x" => *value = "source_names=1:g".to_owned(),
                "canonical_tokens=3:c.x" => {
                    *value = "canonical_tokens=4:1b.g|3:2.g|3:3.g".to_owned();
                }
                "raw_component_operators=1:c" => {
                    *value = "raw_component_operators=2:1b|1:2|1:3".to_owned();
                }
                "raw_time_series_operators=0:" => {
                    *value = "raw_time_series_operators=0:|0:|0:".to_owned();
                }
                _ => {}
            }
        }
        (args, spi)
    }

    fn categorical_interaction_fixture() -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = fixture();
        let sources = ["g", "h"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| SourceEntry {
                id: SourceId::new(u32::try_from(index + 1).expect("small fixture"))
                    .expect("source"),
                canonical_name: name.to_owned(),
            })
            .collect::<Vec<_>>();
        let graph = CompiledTermGraph {
            sources: sources.clone(),
            terms: vec![ExpandedTerm {
                id: TermId::new(1).expect("term"),
                order: TermOrder::new(1).expect("order"),
                coefficient_slot: CoefficientSlot::new(1).expect("slot"),
                raw_kind: RawTermKind::Interaction,
                whole_column_omitted: false,
                canonical_token: "2.g#1.h".to_owned(),
                components: vec![
                    TermComponent {
                        order: ComponentOrder::new(1).expect("component"),
                        source: sources[0].id,
                        kind: ComponentKind::Categorical,
                        level_state: LevelState::Present,
                        level_value: 2.0,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::False,
                        raw_operator: "2".to_owned(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    },
                    TermComponent {
                        order: ComponentOrder::new(2).expect("component"),
                        source: sources[1].id,
                        kind: ComponentKind::Categorical,
                        level_state: LevelState::Present,
                        level_value: 1.0,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::False,
                        raw_operator: "1".to_owned(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    },
                ],
            }],
        };
        let wire = ModelWireV2::from_graph(&graph).expect("interaction wire");
        spi.matrices.insert("S".to_owned(), wire.sources.rows);
        spi.matrices.insert("T".to_owned(), wire.terms.rows);
        spi.matrices.insert("C".to_owned(), wire.components.rows);
        spi.matrices.insert(
            "B".to_owned(),
            vec![
                vec![1.0, 1.0, 0.0],
                vec![2.0, 2.0, 0.0],
                vec![3.0, 3.0, 1.0],
                vec![4.0, 3.0, 2.0],
            ],
        );
        spi.nvars = 7;
        spi.data.insert((3, 1), (-1.0, false));
        for (observation, g, h, h_missing) in [
            (2, 2.0, 1.0, false),
            (3, 1.0, 2.0, true),
            (5, 2.0, 2.0, false),
        ] {
            spi.data.insert((3, observation), (g, false));
            spi.data.insert((4, observation), (h, h_missing));
        }
        for value in &mut args {
            match value.as_str() {
                "source_count=1" => *value = "source_count=2".to_owned(),
                "component_count=1" => *value = "component_count=2".to_owned(),
                "source_names=1:x" => *value = "source_names=1:g|1:h".to_owned(),
                "canonical_tokens=3:c.x" => {
                    *value = "canonical_tokens=7:2.g#1.h".to_owned();
                }
                "raw_component_operators=1:c" => {
                    *value = "raw_component_operators=1:2|1:1".to_owned();
                }
                "raw_time_series_operators=0:" => {
                    *value = "raw_time_series_operators=0:|0:".to_owned();
                }
                "binding_count=3" => *value = "binding_count=4".to_owned(),
                "output_original_observation=4" => {
                    *value = "output_original_observation=5".to_owned();
                }
                "output_retained=5" => *value = "output_retained=6".to_owned(),
                "output_drop_reason=6" => *value = "output_drop_reason=7".to_owned(),
                _ => {}
            }
        }
        (args, spi)
    }

    fn fixed_effect_fixture() -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = fixture();
        let sources = ["x", "g", "h"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| SourceEntry {
                id: SourceId::new(u32::try_from(index + 1).expect("small fixture"))
                    .expect("source"),
                canonical_name: name.to_owned(),
            })
            .collect::<Vec<_>>();
        let model = CompiledTermGraph {
            sources: sources.clone(),
            terms: vec![ExpandedTerm {
                id: TermId::new(1).expect("term"),
                order: TermOrder::new(1).expect("order"),
                coefficient_slot: CoefficientSlot::new(1).expect("slot"),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: "c.x".to_owned(),
                components: vec![TermComponent {
                    order: ComponentOrder::new(1).expect("component"),
                    source: sources[0].id,
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
        let absorbed_terms = [
            (1, sources[1].id, "g"),
            (2, sources[2].id, "h"),
            (3, sources[1].id, "g_again"),
        ]
        .into_iter()
        .map(|(id, source, label)| AbsorbedTerm {
            id: AbsorbTermId::new(id).expect("absorbed term"),
            intercept: true,
            categorical_sources: vec![source],
            slope_sources: Vec::new(),
            label: label.to_owned(),
            saved_name: String::new(),
        })
        .collect::<Vec<_>>();
        let wire = ModelWireV2::from_specification(&CompiledModelSpecification {
            model,
            absorbed_terms,
        })
        .expect("FE wire");
        spi.matrices.insert("S".to_owned(), wire.sources.rows);
        spi.matrices.insert("T".to_owned(), wire.terms.rows);
        spi.matrices.insert("C".to_owned(), wire.components.rows);
        spi.matrices.insert("A".to_owned(), wire.absorb_terms.rows);
        spi.matrices
            .insert("AC".to_owned(), wire.absorb_components.rows);
        spi.matrices.insert(
            "B".to_owned(),
            vec![
                vec![1.0, 1.0, 0.0],
                vec![2.0, 2.0, 0.0],
                vec![3.0, 3.0, 1.0],
                vec![4.0, 3.0, 2.0],
                vec![5.0, 3.0, 3.0],
            ],
        );
        spi.nvars = 8;
        for (observation, g, h, h_missing) in [
            (2, 1.0, 1.5, true),
            (3, 2.0, 2.5, false),
            (5, 3.0, -3.25, false),
        ] {
            spi.data.insert((4, observation), (g, false));
            spi.data.insert((5, observation), (h, h_missing));
        }
        for value in &mut args {
            match value.as_str() {
                "source_count=1" => *value = "source_count=3".to_owned(),
                "source_names=1:x" => *value = "source_names=1:x|1:g|1:h".to_owned(),
                "absorb_term_count=0" => *value = "absorb_term_count=3".to_owned(),
                "absorb_component_count=0" => {
                    *value = "absorb_component_count=3".to_owned();
                }
                "absorb_labels=." => *value = "absorb_labels=1:g|1:h|7:g_again".to_owned(),
                "absorb_saved_names=." => {
                    *value = "absorb_saved_names=0:|0:|0:".to_owned();
                }
                "binding_count=3" => *value = "binding_count=5".to_owned(),
                "keep_singletons=0" => *value = "keep_singletons=1".to_owned(),
                "output_original_observation=4" => {
                    *value = "output_original_observation=6".to_owned();
                }
                "output_retained=5" => *value = "output_retained=7".to_owned(),
                "output_drop_reason=6" => *value = "output_drop_reason=8".to_owned(),
                _ => {}
            }
        }
        (args, spi)
    }

    fn two_way_singleton_fixture() -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = fixed_effect_fixture();
        spi.nobs = 9;
        spi.data.clear();
        for observation in 1..=9 {
            spi.data.insert((1, observation), (1.0, false));
        }
        for (observation, y, x, worker, firm) in [
            (1, 1.2, -1.0, 1.0, 1.0),
            (2, 1.8, -0.5, 1.0, 2.0),
            (3, 2.4, 0.0, 2.0, 2.0),
            (4, 3.1, 0.5, 2.0, 3.0),
            (5, 3.7, 1.0, 3.0, 3.0),
            (6, 4.0, -0.8, 4.0, 4.0),
            (7, 4.5, -0.2, 4.0, 5.0),
            (8, 5.0, 0.3, 5.0, 4.0),
            (9, 5.6, 0.9, 5.0, 5.0),
        ] {
            spi.data.insert((2, observation), (y, false));
            spi.data.insert((3, observation), (x, false));
            spi.data.insert((4, observation), (worker, false));
            spi.data.insert((5, observation), (firm, false));
        }
        for value in &mut args {
            if value == "keep_singletons=1" {
                *value = "keep_singletons=0".to_owned();
            }
        }
        (args, spi)
    }

    fn frequency_singleton_fixture() -> (Vec<String>, FakeSpi) {
        let (mut args, mut spi) = fixture();
        let source = SourceId::new(1).expect("source");
        let wire = ModelWireV2::from_specification(&CompiledModelSpecification {
            model: CompiledTermGraph {
                sources: vec![SourceEntry {
                    id: source,
                    canonical_name: "group".to_owned(),
                }],
                terms: vec![ExpandedTerm {
                    id: TermId::new(1).expect("term"),
                    order: TermOrder::new(1).expect("order"),
                    coefficient_slot: CoefficientSlot::new(1).expect("slot"),
                    raw_kind: RawTermKind::Variable,
                    whole_column_omitted: false,
                    canonical_token: "c.group".to_owned(),
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
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorbed term"),
                intercept: true,
                categorical_sources: vec![source],
                slope_sources: Vec::new(),
                label: "group".to_owned(),
                saved_name: String::new(),
            }],
        })
        .expect("frequency singleton wire");
        spi.matrices.insert("S".to_owned(), wire.sources.rows);
        spi.matrices.insert("T".to_owned(), wire.terms.rows);
        spi.matrices.insert("C".to_owned(), wire.components.rows);
        spi.matrices.insert("A".to_owned(), wire.absorb_terms.rows);
        spi.matrices
            .insert("AC".to_owned(), wire.absorb_components.rows);
        spi.matrices
            .get_mut("B")
            .expect("bindings")
            .push(vec![4.0, 5.0, 0.0]);
        spi.nobs = 2;
        spi.nvars = 7;
        spi.data.clear();
        for observation in 1..=2 {
            spi.data.insert((1, observation), (1.0, false));
        }
        spi.data.insert((2, 1), (1.0, false));
        spi.data.insert((2, 2), (2.0, false));
        spi.data.insert((3, 1), (10.0, false));
        spi.data.insert((3, 2), (20.0, false));
        spi.data.insert((4, 1), (2.0, false));
        spi.data.insert((4, 2), (1.0, false));
        for value in &mut args {
            match value.as_str() {
                "source_names=1:x" => *value = "source_names=5:group".to_owned(),
                "canonical_tokens=3:c.x" => {
                    *value = "canonical_tokens=7:c.group".to_owned();
                }
                "absorb_term_count=0" => *value = "absorb_term_count=1".to_owned(),
                "absorb_component_count=0" => {
                    *value = "absorb_component_count=1".to_owned();
                }
                "absorb_labels=." => *value = "absorb_labels=5:group".to_owned(),
                "absorb_saved_names=." => *value = "absorb_saved_names=0:".to_owned(),
                "binding_count=3" => *value = "binding_count=4".to_owned(),
                "output_original_observation=4" => {
                    *value = "output_original_observation=5".to_owned();
                }
                "output_retained=5" => *value = "output_retained=6".to_owned(),
                "output_drop_reason=6" => *value = "output_drop_reason=7".to_owned(),
                _ => {}
            }
        }
        (args, spi)
    }

    #[test]
    fn successful_action_reads_only_candidate_rows_and_writes_exact_diagnostics() {
        let (args, mut spi) = fixture();
        let output = BufferSink::default();
        prepare_ols_test(&args, &output, &mut spi).expect("prepare action");
        assert_eq!(
            spi.reads,
            vec![
                (1, 1),
                (1, 2),
                (1, 3),
                (1, 4),
                (1, 5),
                (2, 2),
                (3, 2),
                (2, 3),
                (3, 3),
                (2, 5),
                (3, 5),
            ]
        );
        assert_eq!(spi.writes.len(), 15);
        assert_eq!(
            spi.writes
                .iter()
                .filter(|(variable, _, value)| *variable == 5 && *value == 1.0)
                .map(|(_, observation, _)| *observation)
                .collect::<Vec<_>>(),
            vec![2, 5]
        );
        assert!(spi.writes.contains(&(6, 3, 3.0)));
        assert!(output
            .displayed()
            .contains("stfixest.prepare_ols.n_retained=2"));
        assert!(output
            .displayed()
            .contains("stfixest.prepare_ols.n_columns=2"));
    }

    #[test]
    fn factor_main_effect_reads_one_numeric_source_once_and_preserves_atomic_diagnostics() {
        let (args, mut spi) = factor_fixture();
        let output = BufferSink::default();
        prepare_ols_test(&args, &output, &mut spi).expect("factor preparation");
        assert_eq!(
            spi.reads
                .iter()
                .filter(|(variable, _)| *variable == 3)
                .count(),
            3
        );
        assert!(spi
            .reads
            .iter()
            .filter(|(variable, _)| *variable == 3)
            .all(|(_, observation)| matches!(*observation, 2 | 3 | 5)));
        assert!(spi.writes.contains(&(6, 3, 3.0)));
        let displayed = output.displayed();
        assert!(displayed.contains("stfixest.prepare_ols.n_columns=3"));
        assert!(displayed.contains("stfixest.prepare_ols.n_coefficient_slots=4"));
        assert!(displayed.contains("stfixest.prepare_ols.n_categorical_sources=1"));
    }

    #[test]
    fn categorical_interaction_reads_each_numeric_source_once_and_is_candidate_scoped() {
        let (args, mut spi) = categorical_interaction_fixture();
        let output = BufferSink::default();
        prepare_ols_test(&args, &output, &mut spi).expect("interaction preparation");
        for variable in [3, 4] {
            assert_eq!(
                spi.reads
                    .iter()
                    .filter(|(candidate, _)| *candidate == variable)
                    .map(|(_, observation)| *observation)
                    .collect::<Vec<_>>(),
                vec![2, 3, 5]
            );
        }
        assert!(!spi.reads.contains(&(3, 1)));
        assert!(spi.writes.contains(&(7, 3, 3.0)));
        assert!(output
            .displayed()
            .contains("stfixest.prepare_ols.n_columns=2"));
        assert!(output
            .displayed()
            .contains("stfixest.prepare_ols.n_categorical_sources=2"));

        let (args, mut invalid) = categorical_interaction_fixture();
        invalid.data.insert((3, 2), (-1.0, false));
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut invalid)
            .expect_err("candidate interaction source must be valid");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert!(invalid.writes.is_empty());
    }

    #[test]
    fn fixed_effect_sources_are_read_once_and_missingness_uses_reason_five() {
        let (args, mut spi) = fixed_effect_fixture();
        let output = BufferSink::default();
        prepare_ols_test(&args, &output, &mut spi).expect("fixed-effect preparation");
        for variable in [3, 4, 5] {
            assert_eq!(
                spi.reads
                    .iter()
                    .filter(|(candidate, _)| *candidate == variable)
                    .map(|(_, observation)| *observation)
                    .collect::<Vec<_>>(),
                vec![2, 3, 5]
            );
        }
        assert!(spi.writes.contains(&(8, 2, 5.0)));
        assert!(spi.writes.contains(&(8, 3, 3.0)));
        assert!(spi.writes.contains(&(8, 5, 0.0)));
        let displayed = output.displayed();
        assert!(displayed.contains("stfixest.prepare_ols.n_absorb_terms=3"));
        assert!(displayed.contains("stfixest.prepare_ols.n_fixed_effect_sources=2"));
        assert!(displayed.contains("stfixest.prepare_ols.n_retained=1"));
    }

    #[test]
    fn singleton_cascade_and_keepsingletons_write_exact_atomic_diagnostics() {
        let (args, mut spi) = two_way_singleton_fixture();
        let output = BufferSink::default();
        prepare_ols_test(&args, &output, &mut spi).expect("singleton cascade");
        for observation in 1..=5 {
            assert!(spi.writes.contains(&(7, observation, 0.0)));
            assert!(spi.writes.contains(&(8, observation, 8.0)));
        }
        for observation in 6..=9 {
            assert!(spi.writes.contains(&(7, observation, 1.0)));
            assert!(spi.writes.contains(&(8, observation, 0.0)));
        }
        let displayed = output.displayed();
        assert!(displayed.contains("stfixest.prepare_ols.n_retained=4"));
        assert!(displayed.contains("stfixest.prepare_ols.n_singletons=5"));
        assert!(displayed.contains("stfixest.prepare_ols.keep_singletons=0"));

        let (mut keep_args, mut keep_spi) = two_way_singleton_fixture();
        for value in &mut keep_args {
            if value == "keep_singletons=0" {
                *value = "keep_singletons=1".to_owned();
            }
        }
        let keep_output = BufferSink::default();
        prepare_ols_test(&keep_args, &keep_output, &mut keep_spi).expect("keep singletons");
        for observation in 1..=9 {
            assert!(keep_spi.writes.contains(&(7, observation, 1.0)));
            assert!(keep_spi.writes.contains(&(8, observation, 0.0)));
        }
        assert!(keep_output
            .displayed()
            .contains("stfixest.prepare_ols.n_singletons=0"));
        assert!(keep_output
            .displayed()
            .contains("stfixest.prepare_ols.keep_singletons=1"));
    }

    #[test]
    fn frequency_singletons_use_integer_multiplicity_and_bad_policy_fails_before_reads() {
        let (args, mut spi) = frequency_singleton_fixture();
        let output = BufferSink::default();
        prepare_ols_test(&args, &output, &mut spi).expect("frequency singleton");
        assert!(spi.writes.contains(&(6, 1, 1.0)));
        assert!(spi.writes.contains(&(7, 1, 0.0)));
        assert!(spi.writes.contains(&(6, 2, 0.0)));
        assert!(spi.writes.contains(&(7, 2, 8.0)));
        assert!(output
            .displayed()
            .contains("stfixest.prepare_ols.n_singletons=1"));

        let (mut invalid_args, mut invalid_spi) = frequency_singleton_fixture();
        for value in &mut invalid_args {
            if value == "keep_singletons=0" {
                *value = "keep_singletons=2".to_owned();
            }
        }
        let error = prepare_ols_test(&invalid_args, &BufferSink::default(), &mut invalid_spi)
            .expect_err("invalid singleton policy");
        assert_eq!(error.code, ErrorCode::Syntax);
        assert!(invalid_spi.reads.is_empty());
        assert!(invalid_spi.writes.is_empty());
    }

    #[test]
    fn unsupported_fixed_effect_shapes_and_strings_fail_before_observation_reads() {
        let (args, mut string_spi) = fixed_effect_fixture();
        string_spi.strings.insert(4);
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut string_spi)
            .expect_err("string FE must be rejected");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(string_spi.reads.is_empty());
        assert!(string_spi.writes.is_empty());

        let (mut saved_args, mut saved_spi) = fixed_effect_fixture();
        for value in &mut saved_args {
            if value == "absorb_saved_names=0:|0:|0:" {
                *value = "absorb_saved_names=5:alpha|0:|0:".to_owned();
            }
        }
        let error = prepare_ols_test(&saved_args, &BufferSink::default(), &mut saved_spi)
            .expect_err("saved FE must be rejected");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(saved_spi.reads.is_empty());
        assert!(saved_spi.writes.is_empty());

        for role in [AbsorbComponentRole::Categorical, AbsorbComponentRole::Slope] {
            let (mut args, mut spi) = fixed_effect_fixture();
            if role == AbsorbComponentRole::Categorical {
                spi.matrices.get_mut("A").expect("absorb terms")[0][1] = 2.0;
            } else {
                spi.matrices.get_mut("A").expect("absorb terms")[0][3] = 1.0;
            }
            spi.matrices
                .get_mut("AC")
                .expect("absorb components")
                .insert(
                    1,
                    vec![
                        1.0,
                        f64::from(role as u32),
                        if role == AbsorbComponentRole::Categorical {
                            2.0
                        } else {
                            1.0
                        },
                        3.0,
                    ],
                );
            for value in &mut args {
                if value == "absorb_component_count=3" {
                    *value = "absorb_component_count=4".to_owned();
                }
            }
            let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
                .expect_err("unsupported FE shape");
            assert_eq!(error.code, ErrorCode::Unsupported);
            assert!(spi.reads.is_empty());
            assert!(spi.writes.is_empty());
        }
    }

    #[test]
    fn invalid_candidate_factor_value_and_string_source_fail_without_writes() {
        for invalid in [-1.0, 1.5] {
            let (args, mut spi) = factor_fixture();
            spi.data.insert((3, 2), (invalid, false));
            let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
                .expect_err("invalid factor source");
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert!(spi.writes.is_empty());
        }

        let (args, mut spi) = factor_fixture();
        spi.strings.insert(3);
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("string factor source");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());
    }

    #[test]
    fn mixed_interaction_and_pending_time_series_fail_before_observation_reads() {
        let (mut args, mut spi) = factor_fixture();
        spi.matrices.get_mut("C").expect("components")[0][8] = 2.0;
        for value in &mut args {
            if value == "raw_time_series_operators=0:|0:|0:" {
                *value = "raw_time_series_operators=2:L.|0:|0:".to_owned();
            }
        }
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("pending time-series factor");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        let (mut args, mut spi) = fixture();
        let categorical = SourceEntry {
            id: SourceId::new(1).expect("source"),
            canonical_name: "g".to_owned(),
        };
        let continuous = SourceEntry {
            id: SourceId::new(2).expect("source"),
            canonical_name: "x".to_owned(),
        };
        let graph = CompiledTermGraph {
            sources: vec![categorical.clone(), continuous.clone()],
            terms: vec![ExpandedTerm {
                id: TermId::new(1).expect("term"),
                order: TermOrder::new(1).expect("order"),
                coefficient_slot: CoefficientSlot::new(1).expect("slot"),
                raw_kind: RawTermKind::Interaction,
                whole_column_omitted: false,
                canonical_token: "2.g#c.x".to_owned(),
                components: vec![
                    TermComponent {
                        order: ComponentOrder::new(1).expect("component"),
                        source: categorical.id,
                        kind: ComponentKind::Categorical,
                        level_state: LevelState::Present,
                        level_value: 2.0,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::False,
                        raw_operator: "2".to_owned(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    },
                    TermComponent {
                        order: ComponentOrder::new(2).expect("component"),
                        source: continuous.id,
                        kind: ComponentKind::Continuous,
                        level_state: LevelState::Absent,
                        level_value: 0.0,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::Unavailable,
                        raw_operator: "c".to_owned(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    },
                ],
            }],
        };
        let wire = ModelWireV2::from_graph(&graph).expect("interaction wire");
        spi.matrices.insert("S".to_owned(), wire.sources.rows);
        spi.matrices.insert("T".to_owned(), wire.terms.rows);
        spi.matrices.insert("C".to_owned(), wire.components.rows);
        spi.matrices.insert(
            "B".to_owned(),
            vec![
                vec![1.0, 1.0, 0.0],
                vec![2.0, 2.0, 0.0],
                vec![3.0, 3.0, 1.0],
                vec![4.0, 3.0, 2.0],
            ],
        );
        spi.nvars = 7;
        for value in &mut args {
            match value.as_str() {
                "source_count=1" => *value = "source_count=2".to_owned(),
                "component_count=1" => *value = "component_count=2".to_owned(),
                "source_names=1:x" => *value = "source_names=1:g|1:x".to_owned(),
                "canonical_tokens=3:c.x" => {
                    *value = "canonical_tokens=7:2.g#c.x".to_owned();
                }
                "raw_component_operators=1:c" => {
                    *value = "raw_component_operators=1:2|1:c".to_owned();
                }
                "raw_time_series_operators=0:" => {
                    *value = "raw_time_series_operators=0:|0:".to_owned();
                }
                "binding_count=3" => *value = "binding_count=4".to_owned(),
                "output_original_observation=4" => {
                    *value = "output_original_observation=5".to_owned();
                }
                "output_retained=5" => *value = "output_retained=6".to_owned(),
                "output_drop_reason=6" => *value = "output_drop_reason=7".to_owned(),
                _ => {}
            }
        }
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("factor-continuous interaction");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());
    }

    #[test]
    fn analytic_weight_is_read_only_for_candidates_and_controls_the_native_sample() {
        let (args, mut spi) = analytic_weight_fixture();
        // An invalid value outside the candidate sample must never be observed.
        spi.data.insert((4, 1), (-1.0, false));
        let output = BufferSink::default();
        prepare_ols_test(&args, &output, &mut spi).expect("analytic-weight action");
        assert_eq!(
            spi.reads,
            vec![
                (1, 1),
                (1, 2),
                (1, 3),
                (1, 4),
                (1, 5),
                (2, 2),
                (3, 2),
                (4, 2),
                (2, 3),
                (3, 3),
                (4, 3),
                (2, 5),
                (3, 5),
                (4, 5),
            ]
        );
        assert_eq!(
            spi.writes
                .iter()
                .filter(|(variable, _, value)| *variable == 6 && *value == 1.0)
                .map(|(_, observation, _)| *observation)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert!(spi.writes.contains(&(7, 3, 4.0)));
        assert!(spi.writes.contains(&(7, 5, 4.0)));
        assert!(output
            .displayed()
            .contains("stfixest.prepare_ols.n_retained=1"));
    }

    #[test]
    fn candidate_negative_analytic_weight_fails_before_any_write() {
        let (args, mut spi) = analytic_weight_fixture();
        spi.data.insert((4, 2), (-1.0, false));
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("negative analytic weight");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert!(spi.writes.is_empty());

        let (args, mut spi) = analytic_weight_fixture();
        spi.fail_read = Some((4, 3));
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("analytic-weight read failure");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(spi.writes.is_empty());
    }

    #[test]
    fn frequency_weight_is_read_only_for_candidates_and_retains_physical_rows() {
        let (args, mut spi) = frequency_weight_fixture();
        // A fractional value outside the candidate sample must never be read.
        spi.data.insert((4, 1), (1.5, false));
        let output = BufferSink::default();
        prepare_ols_test(&args, &output, &mut spi).expect("frequency-weight action");
        assert_eq!(
            spi.reads,
            vec![
                (1, 1),
                (1, 2),
                (1, 3),
                (1, 4),
                (1, 5),
                (2, 2),
                (3, 2),
                (4, 2),
                (2, 3),
                (3, 3),
                (4, 3),
                (2, 5),
                (3, 5),
                (4, 5),
            ]
        );
        assert_eq!(
            spi.writes
                .iter()
                .filter(|(variable, _, value)| *variable == 6 && *value == 1.0)
                .map(|(_, observation, _)| *observation)
                .collect::<Vec<_>>(),
            vec![2, 3, 5]
        );
        assert!(output
            .displayed()
            .contains("stfixest.prepare_ols.n_retained=3"));
    }

    #[test]
    fn invalid_candidate_frequency_weight_fails_before_any_write() {
        for invalid_weight in [
            0.0,
            -1.0,
            1.5,
            f64::from(u32::MAX) + 1.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            let (args, mut spi) = frequency_weight_fixture();
            spi.data.insert((4, 2), (invalid_weight, false));
            let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
                .expect_err("invalid frequency weight");
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert!(spi.writes.is_empty());
        }

        let (args, mut spi) = frequency_weight_fixture();
        spi.data.insert((4, 2), (f64::NAN, true));
        prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect("explicitly masked frequency weight");
        assert!(spi.writes.contains(&(7, 2, 4.0)));

        let (args, mut spi) = frequency_weight_fixture();
        spi.fail_read = Some((4, 3));
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("frequency-weight read failure");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(spi.writes.is_empty());
    }

    #[test]
    fn probability_weight_is_read_only_for_candidates_and_preserves_physical_rows() {
        let (args, mut spi) = probability_weight_fixture();
        // An invalid value outside the candidate sample must never be read.
        spi.data.insert((4, 1), (-1.0, false));
        let output = BufferSink::default();
        prepare_ols_test(&args, &output, &mut spi).expect("probability-weight action");
        assert_eq!(
            spi.reads,
            vec![
                (1, 1),
                (1, 2),
                (1, 3),
                (1, 4),
                (1, 5),
                (2, 2),
                (3, 2),
                (4, 2),
                (2, 3),
                (3, 3),
                (4, 3),
                (2, 5),
                (3, 5),
                (4, 5),
            ]
        );
        assert_eq!(
            spi.writes
                .iter()
                .filter(|(variable, _, value)| *variable == 6 && *value == 1.0)
                .map(|(_, observation, _)| *observation)
                .collect::<Vec<_>>(),
            vec![2, 3, 5]
        );
        assert!(output
            .displayed()
            .contains("stfixest.prepare_ols.n_retained=3"));
    }

    #[test]
    fn invalid_candidate_probability_weight_fails_before_any_write() {
        for invalid_weight in [0.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let (args, mut spi) = probability_weight_fixture();
            spi.data.insert((4, 2), (invalid_weight, false));
            let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
                .expect_err("invalid probability weight");
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert!(spi.writes.is_empty());
        }

        let (args, mut spi) = probability_weight_fixture();
        spi.data.insert((4, 2), (f64::NAN, true));
        prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect("explicitly masked probability weight");
        assert!(spi.writes.contains(&(7, 2, 4.0)));

        let (args, mut spi) = probability_weight_fixture();
        spi.fail_read = Some((4, 3));
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("probability-weight read failure");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(spi.writes.is_empty());
    }

    #[test]
    fn mixed_weight_bindings_fail_before_observation_reads() {
        let (mut args, mut spi) = analytic_weight_fixture();
        spi.nvars = 8;
        spi.matrices
            .get_mut("B")
            .expect("bindings")
            .push(vec![5.0, 5.0, 0.0]);
        for value in &mut args {
            if value == "binding_count=4" {
                *value = "binding_count=5".to_owned();
            } else if value == "output_original_observation=5" {
                *value = "output_original_observation=6".to_owned();
            } else if value == "output_retained=6" {
                *value = "output_retained=7".to_owned();
            } else if value == "output_drop_reason=7" {
                *value = "output_drop_reason=8".to_owned();
            }
        }
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("mixed weight bindings");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        for other_role in [4.0, 5.0] {
            let (mut args, mut spi) = probability_weight_fixture();
            spi.nvars = 8;
            spi.matrices
                .get_mut("B")
                .expect("bindings")
                .push(vec![5.0, other_role, 0.0]);
            for value in &mut args {
                if value == "binding_count=4" {
                    *value = "binding_count=5".to_owned();
                } else if value == "output_original_observation=5" {
                    *value = "output_original_observation=6".to_owned();
                } else if value == "output_retained=6" {
                    *value = "output_retained=7".to_owned();
                } else if value == "output_drop_reason=7" {
                    *value = "output_drop_reason=8".to_owned();
                }
            }
            let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
                .expect_err("probability weight must be mutually exclusive");
            assert_eq!(error.code, ErrorCode::InvalidArgument);
            assert!(spi.reads.is_empty());
            assert!(spi.writes.is_empty());
        }
    }

    #[test]
    fn malformed_dimensions_and_layout_fail_before_observation_reads() {
        let (args, mut spi) = fixture();
        spi.matrices.get_mut("T").expect("term matrix")[0].pop();
        let error =
            prepare_ols_test(&args, &BufferSink::default(), &mut spi).expect_err("bad dimensions");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        let (args, mut spi) = fixture();
        spi.nvars = 7;
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("bad plugin width");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());
    }

    #[test]
    fn missing_or_nonbinary_candidate_fails_without_diagnostic_writes() {
        for replacement in [(0.0, true), (2.0, false)] {
            let (args, mut spi) = fixture();
            spi.data.insert((1, 3), replacement);
            let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
                .expect_err("bad candidate");
            assert_eq!(error.code, ErrorCode::InvalidData);
            assert!(spi.writes.is_empty());
            assert!(spi.reads.iter().all(|(variable, _)| *variable == 1));
        }
    }

    #[test]
    fn read_failure_and_interruption_fail_without_diagnostic_writes() {
        let (args, mut spi) = fixture();
        spi.fail_read = Some((3, 3));
        let error =
            prepare_ols_test(&args, &BufferSink::default(), &mut spi).expect_err("read failure");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(spi.writes.is_empty());

        let (args, mut spi) = fixture();
        spi.interrupt_at_check = Some(7);
        let error =
            prepare_ols_test(&args, &BufferSink::default(), &mut spi).expect_err("interrupt");
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert!(spi.writes.is_empty());
    }

    #[test]
    fn unsupported_model_and_string_binding_fail_before_observation_reads() {
        let (args, mut spi) = fixture();
        spi.matrices.get_mut("T").expect("term matrix")[0][3] = 2.0;
        let error =
            prepare_ols_test(&args, &BufferSink::default(), &mut spi).expect_err("factor term");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        let (args, mut spi) = fixture();
        spi.strings.insert(2);
        let error =
            prepare_ols_test(&args, &BufferSink::default(), &mut spi).expect_err("string outcome");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        let (args, mut spi) = analytic_weight_fixture();
        spi.strings.insert(4);
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("string analytic weight");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        let (args, mut spi) = analytic_weight_fixture();
        spi.matrices.get_mut("B").expect("bindings")[3][1] = 99.0;
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("unsupported weight binding role");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        let (args, mut spi) = frequency_weight_fixture();
        spi.strings.insert(4);
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("string frequency weight");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        let (args, mut spi) = probability_weight_fixture();
        spi.strings.insert(4);
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("string probability weight");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        let (args, mut spi) = probability_weight_fixture();
        spi.matrices.get_mut("B").expect("bindings")[3][1] = 7.0;
        let error = prepare_ols_test(&args, &BufferSink::default(), &mut spi)
            .expect_err("importance weights remain unsupported");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());
    }

    #[test]
    fn registry_decoder_is_length_delimited_and_exact() {
        assert_eq!(
            decode_registry("0:|3:c.x", 2).expect("registry"),
            vec![String::new(), "c.x".to_owned()]
        );
        assert_eq!(
            decode_registry(".", 0).expect("empty"),
            Vec::<String>::new()
        );
        assert_eq!(
            decode_registry("2:x", 1).expect_err("short entry").code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn developer_confirmation_and_known_fields_are_mandatory() {
        let (mut args, mut spi) = fixture();
        let confirmation = args
            .iter_mut()
            .find(|value| value.starts_with("confirm="))
            .expect("confirmation field");
        *confirmation = "confirm=wrong".to_owned();
        assert_eq!(
            prepare_ols_test(&args, &BufferSink::default(), &mut spi)
                .expect_err("confirmation")
                .code,
            ErrorCode::Unsupported
        );
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());

        let (mut args, mut spi) = fixture();
        args.push("surprise=1".to_owned());
        assert_eq!(
            prepare_ols_test(&args, &BufferSink::default(), &mut spi)
                .expect_err("unknown field")
                .code,
            ErrorCode::Syntax
        );
        assert!(spi.reads.is_empty());
        assert!(spi.writes.is_empty());
    }
}
