use crate::error::{ErrorCode, Result, StFixestError};
use crate::protocol::MODEL_WIRE_VERSION;
use std::collections::BTreeSet;

pub const SOURCE_ROW_WIDTH: usize = 2;
pub const TERM_ROW_WIDTH: usize = 8;
pub const COMPONENT_ROW_WIDTH: usize = 13;
pub const ABSORB_TERM_ROW_WIDTH: usize = 4;
pub const ABSORB_COMPONENT_ROW_WIDTH: usize = 4;

macro_rules! one_based_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            pub fn new(value: u32) -> Result<Self> {
                if value == 0 {
                    Err(invalid(concat!(
                        $label,
                        " is one-based and must be positive"
                    )))
                } else {
                    Ok(Self(value))
                }
            }

            #[must_use]
            pub const fn get(self) -> u32 {
                self.0
            }

            const fn validated(value: u32) -> Self {
                Self(value)
            }
        }
    };
}

one_based_id!(SourceId, "source id");
one_based_id!(TermId, "term id");
one_based_id!(TermOrder, "term order");
one_based_id!(CoefficientSlot, "coefficient slot");
one_based_id!(ComponentOrder, "component order");
one_based_id!(AbsorbTermId, "absorbed-term id");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RawTermKind {
    Variable = 1,
    Factor = 2,
    Interaction = 3,
    Product = 4,
    Unavailable = 5,
}

impl RawTermKind {
    fn from_code(code: u32) -> Result<Self> {
        match code {
            1 => Ok(Self::Variable),
            2 => Ok(Self::Factor),
            3 => Ok(Self::Interaction),
            4 => Ok(Self::Product),
            5 => Ok(Self::Unavailable),
            _ => Err(invalid(format!("unknown raw term kind code {code}"))),
        }
    }

    const fn code(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ComponentKind {
    Continuous = 1,
    Categorical = 2,
    Unavailable = 3,
}

impl ComponentKind {
    fn from_code(code: u32) -> Result<Self> {
        match code {
            1 => Ok(Self::Continuous),
            2 => Ok(Self::Categorical),
            3 => Ok(Self::Unavailable),
            _ => Err(invalid(format!("unknown component kind code {code}"))),
        }
    }

    const fn code(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LevelState {
    Absent = 1,
    Present = 2,
    Unavailable = 3,
}

impl LevelState {
    fn from_code(code: u32) -> Result<Self> {
        match code {
            1 => Ok(Self::Absent),
            2 => Ok(Self::Present),
            3 => Ok(Self::Unavailable),
            _ => Err(invalid(format!("unknown level-state code {code}"))),
        }
    }

    const fn code(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MetadataFlag {
    False = 0,
    True = 1,
    Unavailable = 2,
}

impl MetadataFlag {
    fn from_code(code: u32, label: &str) -> Result<Self> {
        match code {
            0 => Ok(Self::False),
            1 => Ok(Self::True),
            2 => Ok(Self::Unavailable),
            _ => Err(invalid(format!("unknown {label} status code {code}"))),
        }
    }

    const fn code(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TimeSeriesState {
    None = 1,
    PendingMaterialization = 2,
    Materialized = 3,
    Unavailable = 4,
}

impl TimeSeriesState {
    fn from_code(code: u32) -> Result<Self> {
        match code {
            1 => Ok(Self::None),
            2 => Ok(Self::PendingMaterialization),
            3 => Ok(Self::Materialized),
            4 => Ok(Self::Unavailable),
            _ => Err(invalid(format!("unknown time-series state code {code}"))),
        }
    }

    const fn code(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    pub id: SourceId,
    pub canonical_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TermComponent {
    pub order: ComponentOrder,
    pub source: SourceId,
    pub kind: ComponentKind,
    pub level_state: LevelState,
    pub level_value: f64,
    pub base: MetadataFlag,
    pub omitted: MetadataFlag,
    pub raw_operator: String,
    pub time_series_state: TimeSeriesState,
    pub raw_time_series_operator: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedTerm {
    pub id: TermId,
    pub order: TermOrder,
    pub coefficient_slot: CoefficientSlot,
    pub raw_kind: RawTermKind,
    pub whole_column_omitted: bool,
    pub canonical_token: String,
    pub components: Vec<TermComponent>,
}

/// A data-independent graph of already expanded Stata coefficient columns.
///
/// It deliberately contains no base selection, factor-level mapping, intercept
/// insertion, observation data, SPI positions, or executable column recipes.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTermGraph {
    pub sources: Vec<SourceEntry>,
    pub terms: Vec<ExpandedTerm>,
}

impl CompiledTermGraph {
    pub fn validate(&self) -> Result<()> {
        validate_graph(self)
    }

    /// Rejects unavailable metadata that an executable binder must interpret.
    ///
    /// Component-level omission is deliberately not required. Stata does not
    /// define that field for every component shape (notably a plain `c.x`),
    /// and the authoritative omission decision is the term's
    /// `whole_column_omitted` flag.
    pub fn validate_bindable(&self) -> Result<()> {
        self.validate()?;
        for term in &self.terms {
            if term.raw_kind == RawTermKind::Unavailable {
                return Err(unsupported("term kind is unavailable and cannot be bound"));
            }
            for component in &term.components {
                if component.kind == ComponentKind::Unavailable
                    || component.level_state == LevelState::Unavailable
                    || component.base == MetadataFlag::Unavailable
                    || component.time_series_state == TimeSeriesState::Unavailable
                {
                    return Err(unsupported(format!(
                        "term {} component {} has unavailable metadata",
                        term.id.get(),
                        component.order.get()
                    )));
                }
            }
        }
        Ok(())
    }

    /// Also rejects pending Stata time-series materialization. This is not an
    /// estimator compiler and does not produce executable columns.
    pub fn validate_executable_metadata(&self) -> Result<()> {
        self.validate_bindable()?;
        for term in &self.terms {
            for component in &term.components {
                if component.time_series_state == TimeSeriesState::PendingMaterialization {
                    return Err(unsupported(format!(
                        "term {} component {} still requires Stata time-series materialization",
                        term.id.get(),
                        component.order.get()
                    )));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AbsorbComponentRole {
    Categorical = 1,
    Slope = 2,
}

impl AbsorbComponentRole {
    fn from_code(code: u32) -> Result<Self> {
        match code {
            1 => Ok(Self::Categorical),
            2 => Ok(Self::Slope),
            _ => Err(invalid(format!(
                "unknown absorbed-component role code {code}"
            ))),
        }
    }

    const fn code(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbsorbComponent {
    pub role: AbsorbComponentRole,
    pub order: u32,
    pub source: SourceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsorbedTerm {
    pub id: AbsorbTermId,
    pub intercept: bool,
    pub categorical_sources: Vec<SourceId>,
    pub slope_sources: Vec<SourceId>,
    pub label: String,
    pub saved_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledModelSpecification {
    pub model: CompiledTermGraph,
    pub absorbed_terms: Vec<AbsorbedTerm>,
}

impl CompiledModelSpecification {
    pub fn validate(&self) -> Result<()> {
        self.model.validate()?;
        validate_absorbed_terms(&self.absorbed_terms, self.model.sources.len())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NumericWireTable {
    pub declared_count: f64,
    pub rows: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelWireV2 {
    pub major: f64,
    pub minor: f64,
    pub sources: NumericWireTable,
    pub terms: NumericWireTable,
    pub components: NumericWireTable,
    pub source_names: Vec<String>,
    pub canonical_tokens: Vec<String>,
    pub raw_component_operators: Vec<String>,
    pub raw_time_series_operators: Vec<String>,
    pub absorb_terms: NumericWireTable,
    pub absorb_components: NumericWireTable,
    pub absorb_labels: Vec<String>,
    pub absorb_saved_names: Vec<String>,
}

impl ModelWireV2 {
    pub fn from_graph(graph: &CompiledTermGraph) -> Result<Self> {
        graph.validate()?;

        let source_rows = graph
            .sources
            .iter()
            .map(|source| vec![f64::from(source.id.get()), 0.0])
            .collect::<Vec<_>>();
        let mut term_rows = Vec::with_capacity(graph.terms.len());
        let component_count = graph
            .terms
            .iter()
            .map(|term| term.components.len())
            .sum::<usize>();
        let mut component_rows = Vec::with_capacity(component_count);
        let mut raw_component_operators = Vec::with_capacity(component_count);
        let mut raw_time_series_operators = Vec::with_capacity(component_count);
        let mut component_slot = 0_u32;

        for term in &graph.terms {
            term_rows.push(vec![
                f64::from(term.id.get()),
                f64::from(term.order.get()),
                f64::from(term.coefficient_slot.get()),
                f64::from(term.raw_kind.code()),
                term.components.len() as f64,
                if term.whole_column_omitted { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ]);
            for component in &term.components {
                component_slot = component_slot
                    .checked_add(1)
                    .ok_or_else(|| invalid("global component registry exceeds u32"))?;
                component_rows.push(vec![
                    f64::from(term.id.get()),
                    f64::from(component.order.get()),
                    f64::from(component.source.get()),
                    f64::from(component.kind.code()),
                    f64::from(component.level_state.code()),
                    component.level_value,
                    f64::from(component.base.code()),
                    f64::from(component.omitted.code()),
                    f64::from(component.time_series_state.code()),
                    f64::from(component_slot),
                    f64::from(component_slot),
                    0.0,
                    0.0,
                ]);
                raw_component_operators.push(component.raw_operator.clone());
                raw_time_series_operators.push(component.raw_time_series_operator.clone());
            }
        }

        Ok(Self {
            major: f64::from(MODEL_WIRE_VERSION.major),
            minor: f64::from(MODEL_WIRE_VERSION.minor),
            sources: encode_table(source_rows, SOURCE_ROW_WIDTH),
            terms: encode_table(term_rows, TERM_ROW_WIDTH),
            components: encode_table(component_rows, COMPONENT_ROW_WIDTH),
            source_names: graph
                .sources
                .iter()
                .map(|source| source.canonical_name.clone())
                .collect(),
            canonical_tokens: graph
                .terms
                .iter()
                .map(|term| term.canonical_token.clone())
                .collect(),
            raw_component_operators,
            raw_time_series_operators,
            absorb_terms: encode_table(Vec::new(), ABSORB_TERM_ROW_WIDTH),
            absorb_components: encode_table(Vec::new(), ABSORB_COMPONENT_ROW_WIDTH),
            absorb_labels: Vec::new(),
            absorb_saved_names: Vec::new(),
        })
    }

    pub fn from_specification(specification: &CompiledModelSpecification) -> Result<Self> {
        specification.validate()?;
        let mut wire = Self::from_graph(&specification.model)?;
        let mut term_rows = Vec::new();
        let component_count =
            specification
                .absorbed_terms
                .iter()
                .try_fold(0_usize, |total, term| {
                    total
                        .checked_add(term.categorical_sources.len())
                        .and_then(|value| value.checked_add(term.slope_sources.len()))
                        .ok_or_else(|| invalid("absorbed-component cardinality overflows usize"))
                })?;
        term_rows
            .try_reserve_exact(specification.absorbed_terms.len())
            .map_err(|_| allocation("unable to reserve absorbed-term wire rows"))?;
        let mut component_rows = Vec::new();
        component_rows
            .try_reserve_exact(component_count)
            .map_err(|_| allocation("unable to reserve absorbed-component wire rows"))?;
        let mut labels = Vec::new();
        labels
            .try_reserve_exact(specification.absorbed_terms.len())
            .map_err(|_| allocation("unable to reserve absorbed-term labels"))?;
        let mut saved_names = Vec::new();
        saved_names
            .try_reserve_exact(specification.absorbed_terms.len())
            .map_err(|_| allocation("unable to reserve absorbed saved-name registry"))?;

        for term in &specification.absorbed_terms {
            term_rows.push(vec![
                f64::from(term.id.get()),
                term.categorical_sources.len() as f64,
                if term.intercept { 1.0 } else { 0.0 },
                term.slope_sources.len() as f64,
            ]);
            for (index, source) in term.categorical_sources.iter().enumerate() {
                component_rows.push(vec![
                    f64::from(term.id.get()),
                    f64::from(AbsorbComponentRole::Categorical.code()),
                    f64::from(checked_index(index, "absorbed categorical component")?),
                    f64::from(source.get()),
                ]);
            }
            for (index, source) in term.slope_sources.iter().enumerate() {
                component_rows.push(vec![
                    f64::from(term.id.get()),
                    f64::from(AbsorbComponentRole::Slope.code()),
                    f64::from(checked_index(index, "absorbed slope component")?),
                    f64::from(source.get()),
                ]);
            }
            labels.push(term.label.clone());
            saved_names.push(term.saved_name.clone());
        }
        wire.absorb_terms = encode_table(term_rows, ABSORB_TERM_ROW_WIDTH);
        wire.absorb_components = encode_table(component_rows, ABSORB_COMPONENT_ROW_WIDTH);
        wire.absorb_labels = labels;
        wire.absorb_saved_names = saved_names;
        Ok(wire)
    }

    pub fn decode(&self) -> Result<CompiledTermGraph> {
        let specification = self.decode_specification()?;
        if !specification.absorbed_terms.is_empty() {
            return Err(unsupported(
                "model wire contains absorbed terms; decode the complete specification",
            ));
        }
        Ok(specification.model)
    }

    pub fn decode_specification(&self) -> Result<CompiledModelSpecification> {
        let major = positive_u16(self.major, "model-wire major")?;
        let minor = nonnegative_u16(self.minor, "model-wire minor")?;
        if major != MODEL_WIRE_VERSION.major {
            return Err(StFixestError::new(
                ErrorCode::ProtocolMismatch,
                format!(
                    "model-wire major {major} is incompatible with required major {}",
                    MODEL_WIRE_VERSION.major
                ),
            ));
        }
        if minor > MODEL_WIRE_VERSION.minor {
            return Err(StFixestError::new(
                ErrorCode::ProtocolMismatch,
                format!(
                    "model-wire minor {minor} is newer than supported minor {}",
                    MODEL_WIRE_VERSION.minor
                ),
            ));
        }

        let source_rows = normalized_rows(&self.sources, SOURCE_ROW_WIDTH, "source")?;
        let term_rows = normalized_rows(&self.terms, TERM_ROW_WIDTH, "term")?;
        let component_rows = normalized_rows(&self.components, COMPONENT_ROW_WIDTH, "component")?;
        require_registry(&self.source_names, source_rows.len(), "source-name")?;
        require_registry(&self.canonical_tokens, term_rows.len(), "canonical-token")?;
        require_registry(
            &self.raw_component_operators,
            component_rows.len(),
            "raw-component-operator",
        )?;
        require_registry(
            &self.raw_time_series_operators,
            component_rows.len(),
            "raw-time-series-operator",
        )?;

        let mut source_names = BTreeSet::new();
        let mut sources = Vec::with_capacity(source_rows.len());
        for (index, row) in source_rows.iter().enumerate() {
            let expected = checked_index(index, "source")?;
            let id = positive_u32(row[0], "source id")?;
            if id != expected {
                return Err(invalid(format!(
                    "source ids must be contiguous and ordered; expected {expected}, found {id}"
                )));
            }
            require_zero(row[1], "source reserved field")?;
            let name = self.source_names[index].trim();
            if name.is_empty() {
                return Err(invalid("source-name registry contains an empty entry"));
            }
            if !source_names.insert(name.to_owned()) {
                return Err(invalid(format!("duplicate source registry name {name:?}")));
            }
            sources.push(SourceEntry {
                id: SourceId::validated(id),
                canonical_name: name.to_owned(),
            });
        }

        let mut term_headers = Vec::with_capacity(term_rows.len());
        let mut expected_components = 0_usize;
        for (index, row) in term_rows.iter().enumerate() {
            let expected = checked_index(index, "term")?;
            let id = positive_u32(row[0], "term id")?;
            let order = positive_u32(row[1], "term order")?;
            let slot = positive_u32(row[2], "coefficient slot")?;
            if id != expected || order != expected || slot != expected {
                return Err(invalid(format!(
                    "term ids, orders, and coefficient slots must be contiguous and ordered at row {expected}"
                )));
            }
            let raw_kind = RawTermKind::from_code(positive_u32(row[3], "raw term kind")?)?;
            let count = positive_usize(row[4], "term component count")?;
            expected_components = expected_components
                .checked_add(count)
                .ok_or_else(|| invalid("component cardinality overflows usize"))?;
            let whole_column_omitted = strict_bool(row[5], "whole-column omission")?;
            require_zero(row[6], "term reserved field 1")?;
            require_zero(row[7], "term reserved field 2")?;
            let token = self.canonical_tokens[index].trim();
            if token.is_empty() {
                return Err(invalid("canonical-token registry contains an empty entry"));
            }
            term_headers.push((
                id,
                order,
                slot,
                raw_kind,
                count,
                whole_column_omitted,
                token.to_owned(),
            ));
        }
        if expected_components != component_rows.len() {
            return Err(invalid(format!(
                "term component counts total {expected_components}, but component table declares {}",
                component_rows.len()
            )));
        }

        let mut component_index = 0_usize;
        let mut terms = Vec::with_capacity(term_headers.len());
        for (term_index, header) in term_headers.into_iter().enumerate() {
            let (id, order, slot, raw_kind, component_count, whole_omitted, token) = header;
            let mut components = Vec::with_capacity(component_count);
            for local_index in 0..component_count {
                let row = &component_rows[component_index];
                let expected_component = checked_index(local_index, "component")?;
                let row_term = positive_u32(row[0], "component term id")?;
                let component_order = positive_u32(row[1], "component order")?;
                if row_term != id || component_order != expected_component {
                    return Err(invalid(format!(
                        "component rows are orphaned, gapped, or misordered at global row {}",
                        component_index + 1
                    )));
                }
                let source = positive_u32(row[2], "component source id")?;
                if source as usize > sources.len() {
                    return Err(invalid(format!(
                        "component references source id {source} outside the source registry"
                    )));
                }
                let kind = ComponentKind::from_code(positive_u32(row[3], "component kind")?)?;
                let level_state = LevelState::from_code(positive_u32(row[4], "level state")?)?;
                let level_value = finite(row[5], "level value")?;
                let base = MetadataFlag::from_code(
                    nonnegative_u32(row[6], "component base status")?,
                    "component base",
                )?;
                let omitted = MetadataFlag::from_code(
                    nonnegative_u32(row[7], "component omission status")?,
                    "component omission",
                )?;
                let time_series_state =
                    TimeSeriesState::from_code(positive_u32(row[8], "time-series state")?)?;
                let expected_slot = checked_index(component_index, "component registry slot")?;
                let operator_slot = positive_u32(row[9], "raw operator registry slot")?;
                let ts_operator_slot =
                    positive_u32(row[10], "raw time-series operator registry slot")?;
                if operator_slot != expected_slot || ts_operator_slot != expected_slot {
                    return Err(invalid(format!(
                        "component registry slots must be contiguous and ordered at row {expected_slot}"
                    )));
                }
                require_zero(row[11], "component reserved field 1")?;
                require_zero(row[12], "component reserved field 2")?;
                let raw_operator = self.raw_component_operators[component_index].clone();
                let raw_ts_operator = self.raw_time_series_operators[component_index].clone();
                validate_component_semantics(
                    kind,
                    level_state,
                    level_value,
                    base,
                    omitted,
                    &raw_operator,
                    time_series_state,
                    &raw_ts_operator,
                )?;
                components.push(TermComponent {
                    order: ComponentOrder::validated(component_order),
                    source: SourceId::validated(source),
                    kind,
                    level_state,
                    level_value,
                    base,
                    omitted,
                    raw_operator,
                    time_series_state,
                    raw_time_series_operator: raw_ts_operator,
                });
                component_index += 1;
            }
            validate_term_shape(raw_kind, &components, term_index + 1)?;
            terms.push(ExpandedTerm {
                id: TermId::validated(id),
                order: TermOrder::validated(order),
                coefficient_slot: CoefficientSlot::validated(slot),
                raw_kind,
                whole_column_omitted: whole_omitted,
                canonical_token: token,
                components,
            });
        }
        let graph = CompiledTermGraph { sources, terms };
        graph.validate()?;
        let absorbed_terms = decode_absorbed_terms(self, minor, graph.sources.len())?;
        let specification = CompiledModelSpecification {
            model: graph,
            absorbed_terms,
        };
        specification.validate()?;
        Ok(specification)
    }
}

fn decode_absorbed_terms(
    wire: &ModelWireV2,
    minor: u16,
    source_count: usize,
) -> Result<Vec<AbsorbedTerm>> {
    let term_rows = normalized_rows(&wire.absorb_terms, ABSORB_TERM_ROW_WIDTH, "absorbed term")?;
    let component_rows = normalized_rows(
        &wire.absorb_components,
        ABSORB_COMPONENT_ROW_WIDTH,
        "absorbed component",
    )?;
    require_registry(&wire.absorb_labels, term_rows.len(), "absorbed-label")?;
    require_registry(
        &wire.absorb_saved_names,
        term_rows.len(),
        "absorbed-saved-name",
    )?;
    if minor == 0 {
        if !term_rows.is_empty()
            || !component_rows.is_empty()
            || !wire.absorb_labels.is_empty()
            || !wire.absorb_saved_names.is_empty()
        {
            return Err(StFixestError::new(
                ErrorCode::ProtocolMismatch,
                "model-wire 2.0 cannot carry absorbed-term metadata",
            ));
        }
        return Ok(Vec::new());
    }

    let mut headers = Vec::new();
    headers
        .try_reserve_exact(term_rows.len())
        .map_err(|_| allocation("unable to reserve absorbed-term headers"))?;
    let mut expected_components = 0_usize;
    for (index, row) in term_rows.iter().enumerate() {
        let expected_id = checked_index(index, "absorbed term")?;
        let id = positive_u32(row[0], "absorbed-term id")?;
        if id != expected_id {
            return Err(invalid(format!(
                "absorbed-term ids must be contiguous and ordered; expected {expected_id}, found {id}"
            )));
        }
        let n_categorical = positive_usize(row[1], "absorbed categorical count")?;
        let intercept = strict_bool(row[2], "absorbed intercept flag")?;
        let n_slopes = nonnegative_usize(row[3], "absorbed slope count")?;
        if !intercept && n_slopes == 0 {
            return Err(invalid(
                "absorbed term must contain an intercept or at least one slope",
            ));
        }
        expected_components = expected_components
            .checked_add(n_categorical)
            .and_then(|value| value.checked_add(n_slopes))
            .ok_or_else(|| invalid("absorbed-component cardinality overflows usize"))?;
        let label = wire.absorb_labels[index].trim();
        if label.is_empty() {
            return Err(invalid("absorbed-label registry contains an empty entry"));
        }
        let saved_name = wire.absorb_saved_names[index].trim();
        headers.push((
            AbsorbTermId::validated(id),
            n_categorical,
            intercept,
            n_slopes,
            label.to_owned(),
            saved_name.to_owned(),
        ));
    }
    if component_rows.len() != expected_components {
        return Err(invalid(format!(
            "absorbed-term component counts total {expected_components}, but absorbed-component table declares {}",
            component_rows.len()
        )));
    }

    let mut component_index = 0_usize;
    let mut terms = Vec::new();
    terms
        .try_reserve_exact(headers.len())
        .map_err(|_| allocation("unable to reserve absorbed terms"))?;
    for (id, n_categorical, intercept, n_slopes, label, saved_name) in headers {
        let mut categorical_sources = Vec::new();
        categorical_sources
            .try_reserve_exact(n_categorical)
            .map_err(|_| allocation("unable to reserve absorbed categorical sources"))?;
        let mut slope_sources = Vec::new();
        slope_sources
            .try_reserve_exact(n_slopes)
            .map_err(|_| allocation("unable to reserve absorbed slope sources"))?;
        for expected_role in [
            (AbsorbComponentRole::Categorical, n_categorical),
            (AbsorbComponentRole::Slope, n_slopes),
        ] {
            for local_index in 0..expected_role.1 {
                let row = &component_rows[component_index];
                let row_term = positive_u32(row[0], "absorbed-component term id")?;
                let role = AbsorbComponentRole::from_code(positive_u32(
                    row[1],
                    "absorbed-component role",
                )?)?;
                let order = positive_u32(row[2], "absorbed-component order")?;
                let expected_order = checked_index(local_index, "absorbed component")?;
                if row_term != id.get() || role != expected_role.0 || order != expected_order {
                    return Err(invalid(format!(
                        "absorbed components are orphaned, out of role order, or misordered at global row {}",
                        component_index + 1
                    )));
                }
                let source = positive_u32(row[3], "absorbed-component source id")?;
                if source as usize > source_count {
                    return Err(invalid(format!(
                        "absorbed component references source id {source} outside the source registry"
                    )));
                }
                match role {
                    AbsorbComponentRole::Categorical => {
                        categorical_sources.push(SourceId::validated(source));
                    }
                    AbsorbComponentRole::Slope => {
                        slope_sources.push(SourceId::validated(source));
                    }
                }
                component_index += 1;
            }
        }
        terms.push(AbsorbedTerm {
            id,
            intercept,
            categorical_sources,
            slope_sources,
            label,
            saved_name,
        });
    }
    Ok(terms)
}

fn validate_absorbed_terms(terms: &[AbsorbedTerm], source_count: usize) -> Result<()> {
    for (index, term) in terms.iter().enumerate() {
        let expected = checked_index(index, "absorbed term")?;
        if term.id.get() != expected {
            return Err(invalid("absorbed-term ids must be contiguous and ordered"));
        }
        if term.categorical_sources.is_empty() {
            return Err(invalid(
                "absorbed term requires at least one categorical source",
            ));
        }
        if !term.intercept && term.slope_sources.is_empty() {
            return Err(invalid(
                "absorbed term must contain an intercept or at least one slope",
            ));
        }
        if term.label.trim().is_empty() {
            return Err(invalid("absorbed-term label must be nonempty"));
        }
        if term
            .categorical_sources
            .iter()
            .chain(&term.slope_sources)
            .any(|source| source.get() as usize > source_count)
        {
            return Err(invalid(
                "absorbed term references a source outside the source registry",
            ));
        }
    }
    Ok(())
}

fn validate_graph(graph: &CompiledTermGraph) -> Result<()> {
    let source_count = graph.sources.len();
    let mut names = BTreeSet::new();
    for (index, source) in graph.sources.iter().enumerate() {
        let expected = checked_index(index, "source")?;
        if source.id.get() != expected {
            return Err(invalid("source ids must be contiguous and ordered"));
        }
        if source.canonical_name.trim().is_empty()
            || !names.insert(source.canonical_name.trim().to_owned())
        {
            return Err(invalid("source names must be nonempty and unique"));
        }
    }
    for (term_index, term) in graph.terms.iter().enumerate() {
        let expected = checked_index(term_index, "term")?;
        if term.id.get() != expected
            || term.order.get() != expected
            || term.coefficient_slot.get() != expected
        {
            return Err(invalid(
                "term ids, orders, and coefficient slots must be contiguous and ordered",
            ));
        }
        if term.canonical_token.trim().is_empty() || term.components.is_empty() {
            return Err(invalid("terms require a canonical token and components"));
        }
        for (component_index, component) in term.components.iter().enumerate() {
            let expected_component = checked_index(component_index, "component")?;
            if component.order.get() != expected_component
                || component.source.get() as usize > source_count
            {
                return Err(invalid("component order or source id is invalid"));
            }
            validate_component_semantics(
                component.kind,
                component.level_state,
                component.level_value,
                component.base,
                component.omitted,
                &component.raw_operator,
                component.time_series_state,
                &component.raw_time_series_operator,
            )?;
        }
        validate_term_shape(term.raw_kind, &term.components, term_index + 1)?;
    }
    Ok(())
}

fn validate_term_shape(kind: RawTermKind, components: &[TermComponent], row: usize) -> Result<()> {
    let valid = match kind {
        RawTermKind::Variable | RawTermKind::Factor => components.len() == 1,
        RawTermKind::Interaction | RawTermKind::Product => components.len() >= 2,
        RawTermKind::Unavailable => true,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "raw term kind and component count disagree at term row {row}"
        )))
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_component_semantics(
    kind: ComponentKind,
    level_state: LevelState,
    level_value: f64,
    base: MetadataFlag,
    _omitted: MetadataFlag,
    raw_operator: &str,
    ts_state: TimeSeriesState,
    raw_ts_operator: &str,
) -> Result<()> {
    if !level_value.is_finite() {
        return Err(invalid("level value must be finite"));
    }
    if level_state != LevelState::Present && level_value != 0.0 {
        return Err(invalid(
            "absent or unavailable level state requires a zero wire value",
        ));
    }
    match kind {
        ComponentKind::Continuous => {
            if level_state != LevelState::Absent || base != MetadataFlag::False {
                return Err(invalid(
                    "continuous components require an absent level and base=false",
                ));
            }
            if !matches!(raw_operator, "" | "c" | "co") {
                return Err(invalid(format!(
                    "unknown continuous raw component operator {raw_operator:?}"
                )));
            }
        }
        ComponentKind::Categorical => {
            if level_state == LevelState::Absent {
                return Err(invalid(
                    "categorical components require a present or unavailable level",
                ));
            }
            if !valid_categorical_operator(raw_operator) {
                return Err(invalid(format!(
                    "unknown categorical raw component operator {raw_operator:?}"
                )));
            }
        }
        ComponentKind::Unavailable => {
            if level_state != LevelState::Unavailable
                || base != MetadataFlag::Unavailable
                || !raw_operator.is_empty()
            {
                return Err(invalid(
                    "unavailable component kind requires unavailable metadata and an empty raw operator",
                ));
            }
        }
    }
    match ts_state {
        TimeSeriesState::None => {
            if !raw_ts_operator.is_empty() {
                return Err(invalid(
                    "time-series state none requires an empty raw operator",
                ));
            }
        }
        TimeSeriesState::PendingMaterialization | TimeSeriesState::Materialized => {
            if raw_ts_operator.trim().is_empty() {
                return Err(invalid(
                    "pending/materialized time-series state requires a raw operator",
                ));
            }
        }
        TimeSeriesState::Unavailable => {
            if !raw_ts_operator.is_empty() {
                return Err(invalid(
                    "unavailable time-series state requires an empty raw operator",
                ));
            }
        }
    }
    Ok(())
}

fn valid_categorical_operator(operator: &str) -> bool {
    let numeric = operator
        .strip_suffix("bn")
        .or_else(|| operator.strip_suffix('b'))
        .or_else(|| operator.strip_suffix('o'))
        .unwrap_or(operator);
    !numeric.is_empty() && numeric.parse::<f64>().is_ok_and(f64::is_finite)
}

fn encode_table(mut rows: Vec<Vec<f64>>, width: usize) -> NumericWireTable {
    let count = rows.len();
    if rows.is_empty() {
        rows.push(vec![0.0; width]);
    }
    NumericWireTable {
        declared_count: count as f64,
        rows,
    }
}

fn normalized_rows<'a>(
    table: &'a NumericWireTable,
    width: usize,
    label: &str,
) -> Result<&'a [Vec<f64>]> {
    let count = nonnegative_usize(table.declared_count, &format!("{label} row count"))?;
    if table.rows.iter().any(|row| row.len() != width) {
        return Err(invalid(format!(
            "{label} table rows must have width {width}"
        )));
    }
    if count == 0 {
        if table.rows.len() != 1 || table.rows[0].iter().any(|value| *value != 0.0) {
            return Err(invalid(format!(
                "empty {label} table requires exactly one all-zero sentinel row"
            )));
        }
        Ok(&table.rows[..0])
    } else if table.rows.len() != count {
        Err(invalid(format!(
            "{label} table declares {count} rows but carries {}",
            table.rows.len()
        )))
    } else {
        if table.rows.iter().flatten().any(|value| !value.is_finite()) {
            return Err(invalid(format!(
                "{label} table contains a nonfinite numeric field"
            )));
        }
        Ok(&table.rows)
    }
}

fn require_registry(values: &[String], count: usize, label: &str) -> Result<()> {
    if values.len() == count {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} registry has {} entries but requires {count}",
            values.len()
        )))
    }
}

fn finite(value: f64, label: &str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid(format!("{label} must be finite")))
    }
}

fn strict_bool(value: f64, label: &str) -> Result<bool> {
    match finite(value, label)? {
        0.0 => Ok(false),
        1.0 => Ok(true),
        _ => Err(invalid(format!("{label} must be exactly zero or one"))),
    }
}

fn require_zero(value: f64, label: &str) -> Result<()> {
    if finite(value, label)? == 0.0 {
        Ok(())
    } else {
        Err(invalid(format!("{label} must be zero")))
    }
}

fn positive_u16(value: f64, label: &str) -> Result<u16> {
    let integer = positive_u32(value, label)?;
    u16::try_from(integer).map_err(|_| invalid(format!("{label} exceeds u16")))
}

fn nonnegative_u16(value: f64, label: &str) -> Result<u16> {
    let integer = nonnegative_u32(value, label)?;
    u16::try_from(integer).map_err(|_| invalid(format!("{label} exceeds u16")))
}

fn positive_u32(value: f64, label: &str) -> Result<u32> {
    let integer = nonnegative_u32(value, label)?;
    if integer == 0 {
        Err(invalid(format!("{label} must be positive")))
    } else {
        Ok(integer)
    }
}

fn nonnegative_u32(value: f64, label: &str) -> Result<u32> {
    let value = finite(value, label)?;
    if value < 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(invalid(format!(
            "{label} must be a nonnegative integer representable as u32"
        )));
    }
    Ok(value as u32)
}

fn positive_usize(value: f64, label: &str) -> Result<usize> {
    let value = positive_u32(value, label)?;
    usize::try_from(value).map_err(|_| invalid(format!("{label} exceeds usize")))
}

fn nonnegative_usize(value: f64, label: &str) -> Result<usize> {
    let value = nonnegative_u32(value, label)?;
    usize::try_from(value).map_err(|_| invalid(format!("{label} exceeds usize")))
}

fn checked_index(index: usize, label: &str) -> Result<u32> {
    let one_based = index
        .checked_add(1)
        .ok_or_else(|| invalid(format!("{label} index overflow")))?;
    u32::try_from(one_based).map_err(|_| invalid(format!("{label} index exceeds u32")))
}

fn invalid(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
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

    #[test]
    fn one_based_ids_reject_zero() {
        assert_eq!(
            SourceId::new(0).expect_err("zero must fail").code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(SourceId::new(1).expect("one is valid").get(), 1);
    }

    #[test]
    fn empty_graph_round_trips_through_explicit_sentinels() {
        let graph = CompiledTermGraph {
            sources: Vec::new(),
            terms: Vec::new(),
        };
        let wire = ModelWireV2::from_graph(&graph).expect("valid empty graph");
        assert_eq!(wire.sources.rows, vec![vec![0.0; SOURCE_ROW_WIDTH]]);
        assert_eq!(wire.terms.rows, vec![vec![0.0; TERM_ROW_WIDTH]]);
        assert_eq!(wire.components.rows, vec![vec![0.0; COMPONENT_ROW_WIDTH]]);
        assert_eq!(wire.decode().expect("round trip"), graph);
    }

    #[test]
    fn bindable_continuous_component_may_have_inapplicable_omission_metadata() {
        let graph = CompiledTermGraph {
            sources: vec![SourceEntry {
                id: SourceId::new(1).expect("id"),
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
                    source: SourceId::new(1).expect("source"),
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
        graph.validate_bindable().expect("plain c.x must bind");
    }
}
