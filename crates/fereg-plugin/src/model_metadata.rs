//! One host model descriptor; transport framing and registry transfer are explicit.
use crate::input::SpiApi;
const BINDING_ROW_WIDTH: usize = 3;
use fereg_core::error::{allocation, invalid_argument as invalid, ErrorCode, FeregError, Result};
use fereg_core::metadata::{
    ModelWireV2, NumericWireTable, SourceId, ABSORB_COMPONENT_ROW_WIDTH, ABSORB_TERM_ROW_WIDTH,
    COMPONENT_ROW_WIDTH, SOURCE_ROW_WIDTH, TERM_ROW_WIDTH,
};
use fereg_core::ols::{OlsInputBinding, OlsInputRole};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct ModelRegistries {
    pub(crate) source_names: Vec<String>,
    pub(crate) canonical_tokens: Vec<String>,
    pub(crate) raw_component_operators: Vec<String>,
    pub(crate) raw_time_series_operators: Vec<String>,
    pub(crate) absorb_labels: Vec<String>,
    pub(crate) absorb_saved_names: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct ModelMetadata {
    pub(crate) model_major: f64,
    pub(crate) model_minor: f64,
    pub(crate) source_matrix: String,
    pub(crate) source_count: usize,
    pub(crate) term_matrix: String,
    pub(crate) term_count: usize,
    pub(crate) component_matrix: String,
    pub(crate) component_count: usize,
    pub(crate) absorb_term_matrix: String,
    pub(crate) absorb_term_count: usize,
    pub(crate) absorb_component_matrix: String,
    pub(crate) absorb_component_count: usize,
    pub(crate) registries: ModelRegistries,
}

#[derive(Clone, Copy)]
pub(crate) enum RegistryFraming {
    Delimited,
    ByteLengths,
}

impl ModelMetadata {
    pub(crate) fn counts(values: &BTreeMap<String, String>) -> Result<[usize; 5]> {
        Ok([
            parse_usize(values, "source_count")?,
            parse_usize(values, "term_count")?,
            parse_usize(values, "component_count")?,
            parse_usize(values, "absorb_term_count")?,
            parse_usize(values, "absorb_component_count")?,
        ])
    }
    pub(crate) fn parse(
        values: &BTreeMap<String, String>,
        counts: [usize; 5],
        framing: RegistryFraming,
    ) -> Result<Self> {
        let [source_count, term_count, component_count, absorb_term_count, absorb_component_count] =
            counts;
        Ok(Self {
            model_major: f64::from(parse_u16(values, "model_major")?),
            model_minor: f64::from(parse_u16(values, "model_minor")?),
            source_matrix: required(values, "source_matrix")?.to_owned(),
            source_count,
            term_matrix: required(values, "term_matrix")?.to_owned(),
            term_count,
            component_matrix: required(values, "component_matrix")?.to_owned(),
            component_count,

            absorb_term_matrix: required(values, "absorb_term_matrix")?.to_owned(),
            absorb_term_count,
            absorb_component_matrix: required(values, "absorb_component_matrix")?.to_owned(),
            absorb_component_count,

            registries: ModelRegistries {
                source_names: decode_registry(
                    required(values, "source_names")?,
                    source_count,
                    framing,
                )?,
                canonical_tokens: decode_registry(
                    required(values, "canonical_tokens")?,
                    term_count,
                    framing,
                )?,
                raw_component_operators: decode_registry(
                    required(values, "raw_component_operators")?,
                    component_count,
                    framing,
                )?,
                raw_time_series_operators: decode_registry(
                    required(values, "raw_time_series_operators")?,
                    component_count,
                    framing,
                )?,
                absorb_labels: decode_registry(
                    required(values, "absorb_labels")?,
                    absorb_term_count,
                    framing,
                )?,
                absorb_saved_names: decode_registry(
                    required(values, "absorb_saved_names")?,
                    absorb_term_count,
                    framing,
                )?,
            },
        })
    }
    pub(crate) fn read_wire(
        &self,
        spi: &mut dyn SpiApi,
        registries: ModelRegistries,
        labels: [&str; 5],
    ) -> Result<ModelWireV2> {
        Ok(ModelWireV2 {
            major: self.model_major,
            minor: self.model_minor,
            sources: read_table(
                spi,
                &self.source_matrix,
                self.source_count,
                SOURCE_ROW_WIDTH,
                labels[0],
            )?,
            terms: read_table(
                spi,
                &self.term_matrix,
                self.term_count,
                TERM_ROW_WIDTH,
                labels[1],
            )?,
            components: read_table(
                spi,
                &self.component_matrix,
                self.component_count,
                COMPONENT_ROW_WIDTH,
                labels[2],
            )?,
            source_names: registries.source_names,
            canonical_tokens: registries.canonical_tokens,
            raw_component_operators: registries.raw_component_operators,
            raw_time_series_operators: registries.raw_time_series_operators,
            absorb_terms: read_table(
                spi,
                &self.absorb_term_matrix,
                self.absorb_term_count,
                ABSORB_TERM_ROW_WIDTH,
                labels[3],
            )?,
            absorb_components: read_table(
                spi,
                &self.absorb_component_matrix,
                self.absorb_component_count,
                ABSORB_COMPONENT_ROW_WIDTH,
                labels[4],
            )?,
            absorb_labels: registries.absorb_labels,
            absorb_saved_names: registries.absorb_saved_names,
        })
    }
}

pub(crate) fn decode_registry(
    value: &str,
    expected: usize,
    framing: RegistryFraming,
) -> Result<Vec<String>> {
    let invalid = |message: &str| FeregError::new(ErrorCode::InvalidArgument, message);
    if expected == 0 {
        return if value == "." {
            Ok(Vec::new())
        } else {
            Err(invalid("empty registry must use the dot sentinel"))
        };
    }
    let delimited = matches!(framing, RegistryFraming::Delimited);
    if delimited && value.split('|').count() != expected {
        return Err(invalid(
            "registry cardinality does not match its declared count",
        ));
    }
    // Declared counts alone cannot justify allocation. Byte-framed developer
    // requests can contain delimiters inside payloads, so bound by encoded size.
    if expected > value.len() / 2 {
        return Err(invalid("registry cardinality exceeds encoded framing"));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(expected)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, "unable to reserve model registry"))?;
    let mut rest = value;
    while !rest.is_empty() {
        if output.len() == expected {
            return Err(invalid(
                "registry cardinality does not match its declared count",
            ));
        }
        let (length, payload) = rest
            .split_once(':')
            .ok_or_else(|| invalid("registry item lacks a length prefix"))?;
        let length = length.parse::<usize>().map_err(|_| {
            FeregError::new(
                if delimited {
                    ErrorCode::Syntax
                } else {
                    ErrorCode::InvalidArgument
                },
                "registry length must be an unsigned integer",
            )
        })?;
        let text = payload
            .get(..length)
            .ok_or_else(|| invalid("registry byte length is incorrect"))?;
        if delimited && text.contains('|') {
            return Err(invalid("registry byte length is incorrect"));
        }
        let mut owned = String::new();
        owned.try_reserve_exact(length).map_err(|_| {
            FeregError::new(ErrorCode::Allocation, "unable to reserve registry payload")
        })?;
        owned.push_str(text);
        output.push(owned);
        rest = &payload[length..];
        if !rest.is_empty() {
            rest = rest
                .strip_prefix('|')
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    invalid("registry entries must be separated by a single delimiter")
                })?;
        }
    }
    if output.len() != expected {
        return Err(invalid(
            "registry cardinality does not match its declared count",
        ));
    }
    Ok(output)
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

pub(crate) fn exact_usize(value: f64, label: &str) -> Result<usize> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return Err(invalid(format!(
            "{label} must be an exact nonnegative integer"
        )));
    }
    Ok(value as usize)
}

pub(crate) fn exact_positive_u32(value: f64, label: &str) -> Result<u32> {
    if !value.is_finite() || value < 1.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(invalid(format!("{label} must be an exact positive u32")));
    }
    Ok(value as u32)
}

pub(crate) fn reserved_empty<T>(elements: usize, label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(format!("unable to reserve {label}")))?;
    if output.capacity() < elements {
        return Err(allocation(format!("{label} reservation was undersized")));
    }
    Ok(output)
}

pub(crate) fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

pub(crate) fn syntax(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Syntax, message)
}

/// An exactly reserved vector of `elements` copies of `value`.
pub(crate) fn reserved_filled<T: Clone>(
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = reserved_empty(elements, label)?;
    output.resize(elements, value);
    Ok(output)
}

pub(crate) fn checked_product(left: usize, right: usize, label: &'static str) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| allocation(format!("{label} size overflows")))
}
