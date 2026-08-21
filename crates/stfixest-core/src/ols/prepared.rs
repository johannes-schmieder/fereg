use std::collections::{BTreeMap, BTreeSet};

use crate::data::{
    prepare_numeric_sample, AdjustmentInput, DomainDecision, NumericColumnView,
    NumericDomainPolicy, NumericPreparationInput, PreparedNumericSample, WeightInput, WeightKind,
};
use crate::error::{ErrorCode, Result, StFixestError};
use crate::factorize::{factorize_f64, EncodedFactor};
use crate::metadata::{
    CoefficientSlot, CompiledTermGraph, ComponentKind, LevelState, MetadataFlag, RawTermKind,
    SourceId, TimeSeriesState,
};

/// Role of one Stata plugin-varlist input in the developer OLS preparation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OlsInputRole {
    Candidate,
    Outcome,
    /// One numeric model source. A single source may be used by both `c.x`
    /// and `i.x`; the binding therefore describes storage, not term meaning.
    NumericSource(SourceId),
    AnalyticWeight,
    FrequencyWeight,
    ProbabilityWeight,
}

/// One-based binding from semantic input role to Stata's plugin varlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlsInputBinding {
    pub stata_var_position: u32,
    pub role: OlsInputRole,
}

/// Version-one request for the deliberately narrow OLS preparation boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct OlsRequestV1 {
    pub model: CompiledTermGraph,
    pub bindings: Vec<OlsInputBinding>,
    pub include_intercept: bool,
}

/// Raw numeric source read by the plugin, retaining Stata missing metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedNumericColumn {
    pub values: Vec<f64>,
    pub missing: Vec<bool>,
}

/// Complete raw input for the numeric-source, optionally weighted, no-FE
/// preparation slice. At most one weight column may be present.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedOlsInput {
    pub candidate: Vec<bool>,
    pub outcome: LoadedNumericColumn,
    pub numeric_sources: Vec<(SourceId, LoadedNumericColumn)>,
    pub analytic_weight: Option<LoadedNumericColumn>,
    pub frequency_weight: Option<LoadedNumericColumn>,
    pub probability_weight: Option<LoadedNumericColumn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsColumnKind {
    Intercept,
    Continuous {
        source: SourceId,
        prepared_source_index: usize,
    },
    Categorical {
        source: SourceId,
        prepared_source_index: usize,
        categorical_source_index: usize,
        /// Canonicalized Stata level value, retained as bits so the recipe is
        /// exactly comparable without making an `f64` enum non-`Eq`.
        level_bits: u64,
        /// `None` means Stata expanded this level in the candidate universe,
        /// but the level disappeared after authoritative sample preparation.
        /// The executable column is then identically zero.
        level_code: Option<u32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsColumn {
    pub coefficient_slot: CoefficientSlot,
    pub kind: PreparedOlsColumnKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsCoefficientStatus {
    Active,
    SyntaxOmitted,
}

/// One Stata-facing coefficient slot. Generated columns are stored separately
/// so an omitted base remains represented without entering numerical work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsCoefficient {
    pub coefficient_slot: CoefficientSlot,
    pub status: PreparedOlsCoefficientStatus,
    pub generated_column_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOlsModel {
    pub coefficients: Vec<PreparedOlsCoefficient>,
    pub columns: Vec<PreparedOlsColumn>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedCategoricalSource {
    pub source: SourceId,
    pub prepared_source_index: usize,
    pub encoded: EncodedFactor<f64>,
}

#[derive(Debug, PartialEq)]
pub struct PreparedOlsData {
    pub model: PreparedOlsModel,
    pub sample: PreparedNumericSample,
    pub categorical_sources: Vec<PreparedCategoricalSource>,
}

impl PreparedOlsData {
    /// Evaluate one prepared finite-dimensional recipe at one active row.
    /// This is a testable preparation primitive, not an estimator dispatch.
    pub fn column_value(&self, column: usize, active_row: usize) -> Result<f64> {
        if active_row >= self.sample.n_active() {
            return Err(invalid("prepared OLS active row is out of range"));
        }
        let column = self
            .model
            .columns
            .get(column)
            .ok_or_else(|| invalid("prepared OLS column is out of range"))?;
        match column.kind {
            PreparedOlsColumnKind::Intercept => Ok(1.0),
            PreparedOlsColumnKind::Continuous {
                prepared_source_index,
                ..
            } => self
                .sample
                .regressors
                .get(prepared_source_index)
                .and_then(|source| source.get(active_row))
                .copied()
                .ok_or_else(|| invalid("prepared continuous source is out of range")),
            PreparedOlsColumnKind::Categorical {
                categorical_source_index,
                level_code,
                ..
            } => {
                let Some(level_code) = level_code else {
                    return Ok(0.0);
                };
                let code = self
                    .categorical_sources
                    .get(categorical_source_index)
                    .and_then(|source| source.encoded.codes.get(active_row))
                    .copied()
                    .ok_or_else(|| invalid("prepared categorical source is out of range"))?;
                Ok(if code == level_code { 1.0 } else { 0.0 })
            }
        }
    }
}

impl OlsRequestV1 {
    /// Validate the entire request and compile the currently supported plain
    /// continuous and numeric categorical main-effect recipes. This performs
    /// no observation reads; categorical level codes are resolved later.
    pub fn validate_and_compile(&self) -> Result<PreparedOlsModel> {
        self.model.validate_executable_metadata()?;
        validate_bindings(self)?;

        let categorical_source_indices = categorical_source_indices(&self.model)?;
        let mut columns = Vec::new();
        let mut coefficients = Vec::new();
        columns
            .try_reserve_exact(
                self.model
                    .terms
                    .len()
                    .checked_add(usize::from(self.include_intercept))
                    .ok_or_else(|| allocation("prepared OLS column count overflows"))?,
            )
            .map_err(|_| allocation("unable to reserve prepared OLS columns"))?;
        coefficients
            .try_reserve_exact(
                self.model
                    .terms
                    .len()
                    .checked_add(usize::from(self.include_intercept))
                    .ok_or_else(|| allocation("prepared OLS coefficient count overflows"))?,
            )
            .map_err(|_| allocation("unable to reserve prepared OLS coefficients"))?;

        for term in &self.model.terms {
            if term.components.len() != 1 {
                return Err(unsupported(
                    "prepare-ols-test supports only plain continuous and factor main-effect terms",
                ));
            }
            let component = &term.components[0];
            let kind = match (term.raw_kind, component.kind) {
                (RawTermKind::Variable, ComponentKind::Continuous)
                    if component.level_state == LevelState::Absent
                        && component.base == MetadataFlag::False
                        && component.time_series_state == TimeSeriesState::None
                        && matches!(component.raw_operator.as_str(), "" | "c" | "co") =>
                {
                    PreparedOlsColumnKind::Continuous {
                        source: component.source,
                        prepared_source_index: source_index(component.source)?,
                    }
                }
                (RawTermKind::Factor, ComponentKind::Categorical)
                    if component.level_state == LevelState::Present
                        && component.time_series_state == TimeSeriesState::None =>
                {
                    validate_categorical_value(component.level_value, "factor metadata level")?;
                    PreparedOlsColumnKind::Categorical {
                        source: component.source,
                        prepared_source_index: source_index(component.source)?,
                        categorical_source_index: *categorical_source_indices
                            .get(&component.source)
                            .ok_or_else(|| invalid("categorical source plan disappeared"))?,
                        level_bits: canonical_factor_value(component.level_value).to_bits(),
                        level_code: None,
                    }
                }
                _ => {
                    return Err(unsupported(
                        "prepare-ols-test supports only non-time-series continuous and numeric factor main-effect terms",
                    ));
                }
            };

            // Never infer whole-column omission from component metadata. Stata's
            // term flag is the sole authority for whether this recipe is emitted.
            if !term.whole_column_omitted {
                let generated_column_index = columns.len();
                columns.push(PreparedOlsColumn {
                    coefficient_slot: term.coefficient_slot,
                    kind,
                });
                coefficients.push(PreparedOlsCoefficient {
                    coefficient_slot: term.coefficient_slot,
                    status: PreparedOlsCoefficientStatus::Active,
                    generated_column_index: Some(generated_column_index),
                });
            } else {
                coefficients.push(PreparedOlsCoefficient {
                    coefficient_slot: term.coefficient_slot,
                    status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                    generated_column_index: None,
                });
            }
        }

        if self.include_intercept {
            let next = self
                .model
                .terms
                .len()
                .checked_add(1)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| invalid("intercept coefficient slot exceeds u32"))?;
            columns.push(PreparedOlsColumn {
                coefficient_slot: CoefficientSlot::new(next)?,
                kind: PreparedOlsColumnKind::Intercept,
            });
            coefficients.push(PreparedOlsCoefficient {
                coefficient_slot: CoefficientSlot::new(next)?,
                status: PreparedOlsCoefficientStatus::Active,
                generated_column_index: Some(columns.len() - 1),
            });
        }
        Ok(PreparedOlsModel {
            coefficients,
            columns,
        })
    }
}

/// Finish the Stata-independent boundary after the SPI layer has loaded raw inputs.
pub fn prepare_ols_data(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedOlsData> {
    let mut model = request.validate_and_compile()?;
    validate_loaded(request, loaded)?;
    validate_candidate_categorical_domain(request, loaded)?;

    let regressor_views = loaded
        .numeric_sources
        .iter()
        .map(|(_, column)| NumericColumnView::with_missing(&column.values, &column.missing))
        .collect::<Vec<_>>();
    validate_candidate_weight_domain(loaded)?;
    let weights = match (
        &loaded.analytic_weight,
        &loaded.frequency_weight,
        &loaded.probability_weight,
    ) {
        (None, None, None) => WeightInput::None,
        (Some(column), None, None) => WeightInput::Provided {
            kind: WeightKind::Analytic,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        },
        (None, Some(column), None) => WeightInput::Provided {
            kind: WeightKind::Frequency,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        },
        (None, None, Some(column)) => WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::with_missing(&column.values, &column.missing),
        },
        _ => {
            return Err(invalid(
                "loaded OLS input cannot contain more than one weight column",
            ));
        }
    };
    let input = NumericPreparationInput {
        candidate: &loaded.candidate,
        outcome: NumericColumnView::with_missing(&loaded.outcome.values, &loaded.outcome.missing),
        regressors: &regressor_views,
        weights,
        adjustment: AdjustmentInput::None,
    };
    let sample = prepare_numeric_sample(&input, &OlsPreparationPolicy)?;
    let categorical_sources = factorize_categorical_sources(request, &sample)?;
    resolve_categorical_level_codes(&mut model, &categorical_sources)?;
    Ok(PreparedOlsData {
        model,
        sample,
        categorical_sources,
    })
}

fn categorical_source_indices(model: &CompiledTermGraph) -> Result<BTreeMap<SourceId, usize>> {
    let categorical = model
        .terms
        .iter()
        .flat_map(|term| &term.components)
        .filter(|component| component.kind == ComponentKind::Categorical)
        .map(|component| component.source)
        .collect::<BTreeSet<_>>();
    let mut indices = BTreeMap::new();
    for source in &model.sources {
        if categorical.contains(&source.id) {
            indices.insert(source.id, indices.len());
        }
    }
    Ok(indices)
}

fn validate_candidate_categorical_domain(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<()> {
    let categorical = categorical_source_indices(&request.model)?;
    for (source, column) in &loaded.numeric_sources {
        if !categorical.contains_key(source) {
            continue;
        }
        for (row, candidate) in loaded.candidate.iter().copied().enumerate() {
            if !candidate || column.missing[row] || !column.values[row].is_finite() {
                continue;
            }
            validate_categorical_value(
                column.values[row],
                &format!(
                    "numeric categorical source {} at observation {}",
                    source.get(),
                    row + 1
                ),
            )?;
        }
    }
    Ok(())
}

fn validate_categorical_value(value: f64, label: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
        return Err(invalid_data(format!(
            "{label} must be a finite nonnegative integer",
        )));
    }
    Ok(())
}

fn canonical_factor_value(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn factorize_categorical_sources(
    request: &OlsRequestV1,
    sample: &PreparedNumericSample,
) -> Result<Vec<PreparedCategoricalSource>> {
    let categorical = categorical_source_indices(&request.model)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(categorical.len())
        .map_err(|_| allocation("unable to reserve prepared categorical sources"))?;
    for source in &request.model.sources {
        if !categorical.contains_key(&source.id) {
            continue;
        }
        let prepared_source_index = source_index(source.id)?;
        let values = sample
            .regressors
            .get(prepared_source_index)
            .ok_or_else(|| invalid("prepared categorical source index is out of range"))?;
        output.push(PreparedCategoricalSource {
            source: source.id,
            prepared_source_index,
            encoded: factorize_f64(values)?,
        });
    }
    Ok(output)
}

fn resolve_categorical_level_codes(
    model: &mut PreparedOlsModel,
    categorical_sources: &[PreparedCategoricalSource],
) -> Result<()> {
    for column in &mut model.columns {
        let PreparedOlsColumnKind::Categorical {
            categorical_source_index,
            level_bits,
            level_code,
            ..
        } = &mut column.kind
        else {
            continue;
        };
        let source = categorical_sources
            .get(*categorical_source_index)
            .ok_or_else(|| invalid("prepared categorical source plan is out of range"))?;
        let level = f64::from_bits(*level_bits);
        *level_code = source
            .encoded
            .levels
            .binary_search_by(|candidate| candidate.total_cmp(&level))
            .ok()
            .map(|index| {
                u32::try_from(index)
                    .map_err(|_| invalid("prepared categorical level code exceeds u32"))
            })
            .transpose()?;
    }
    Ok(())
}

fn source_index(source: SourceId) -> Result<usize> {
    usize::try_from(source.get() - 1).map_err(|_| invalid("source id does not fit usize"))
}

fn validate_bindings(request: &OlsRequestV1) -> Result<()> {
    let mut positions = BTreeSet::new();
    let mut candidate_count = 0_usize;
    let mut outcome_count = 0_usize;
    let mut analytic_weight_count = 0_usize;
    let mut frequency_weight_count = 0_usize;
    let mut probability_weight_count = 0_usize;
    let mut sources = BTreeMap::new();
    for binding in &request.bindings {
        if binding.stata_var_position == 0 || !positions.insert(binding.stata_var_position) {
            return Err(invalid(
                "OLS input bindings require unique positive Stata varlist positions",
            ));
        }
        match binding.role {
            OlsInputRole::Candidate => candidate_count += 1,
            OlsInputRole::Outcome => outcome_count += 1,
            OlsInputRole::NumericSource(source) => {
                if sources.insert(source, binding.stata_var_position).is_some() {
                    return Err(invalid("numeric source is bound more than once"));
                }
            }
            OlsInputRole::AnalyticWeight => analytic_weight_count += 1,
            OlsInputRole::FrequencyWeight => frequency_weight_count += 1,
            OlsInputRole::ProbabilityWeight => probability_weight_count += 1,
        }
    }
    if candidate_count != 1 || outcome_count != 1 {
        return Err(invalid(
            "OLS request requires exactly one candidate and one outcome binding",
        ));
    }
    if analytic_weight_count + frequency_weight_count + probability_weight_count > 1 {
        return Err(invalid("OLS request permits at most one weight binding"));
    }
    if sources.len() != request.model.sources.len()
        || request
            .model
            .sources
            .iter()
            .any(|source| !sources.contains_key(&source.id))
    {
        return Err(invalid(
            "OLS request must bind every model source exactly once",
        ));
    }
    Ok(())
}

fn validate_loaded(request: &OlsRequestV1, loaded: &LoadedOlsInput) -> Result<()> {
    let n = loaded.candidate.len();
    validate_loaded_column("outcome", &loaded.outcome, n)?;
    if loaded.numeric_sources.len() != request.model.sources.len() {
        return Err(invalid(
            "loaded numeric source count does not match the model registry",
        ));
    }
    for (index, (source, column)) in loaded.numeric_sources.iter().enumerate() {
        if *source != request.model.sources[index].id {
            return Err(invalid(
                "loaded numeric sources are not in model-registry order",
            ));
        }
        validate_loaded_column("numeric source", column, n)?;
    }
    let request_has_analytic_weight = request
        .bindings
        .iter()
        .any(|binding| binding.role == OlsInputRole::AnalyticWeight);
    if request_has_analytic_weight != loaded.analytic_weight.is_some() {
        return Err(invalid(
            "loaded analytic weight does not match the OLS request binding",
        ));
    }
    if let Some(column) = &loaded.analytic_weight {
        validate_loaded_column("analytic weight", column, n)?;
    }
    let request_has_frequency_weight = request
        .bindings
        .iter()
        .any(|binding| binding.role == OlsInputRole::FrequencyWeight);
    if request_has_frequency_weight != loaded.frequency_weight.is_some() {
        return Err(invalid(
            "loaded frequency weight does not match the OLS request binding",
        ));
    }
    if let Some(column) = &loaded.frequency_weight {
        validate_loaded_column("frequency weight", column, n)?;
    }
    let request_has_probability_weight = request
        .bindings
        .iter()
        .any(|binding| binding.role == OlsInputRole::ProbabilityWeight);
    if request_has_probability_weight != loaded.probability_weight.is_some() {
        return Err(invalid(
            "loaded probability weight does not match the OLS request binding",
        ));
    }
    if let Some(column) = &loaded.probability_weight {
        validate_loaded_column("probability weight", column, n)?;
    }
    if usize::from(loaded.analytic_weight.is_some())
        + usize::from(loaded.frequency_weight.is_some())
        + usize::from(loaded.probability_weight.is_some())
        > 1
    {
        return Err(invalid(
            "loaded OLS input cannot contain more than one weight column",
        ));
    }
    Ok(())
}

fn validate_candidate_weight_domain(loaded: &LoadedOlsInput) -> Result<()> {
    let Some((kind, weight)) = loaded_weight(loaded) else {
        return Ok(());
    };
    for (row, candidate) in loaded.candidate.iter().copied().enumerate() {
        if !candidate || weight.missing[row] {
            continue;
        }
        let value = weight.values[row];
        match kind {
            WeightKind::Analytic if value < 0.0 => {
                return Err(invalid_data(format!(
                    "analytic weight at observation {} is negative",
                    row + 1,
                )));
            }
            WeightKind::Frequency
                if !value.is_finite()
                    || value <= 0.0
                    || value.fract() != 0.0
                    || value > f64::from(u32::MAX) =>
            {
                return Err(invalid_data(format!(
                    "frequency weight at observation {} must be a positive integer representable as u32",
                    row + 1,
                )));
            }
            WeightKind::Probability if !value.is_finite() || value <= 0.0 => {
                return Err(invalid_data(format!(
                    "probability weight at observation {} must be positive and finite",
                    row + 1,
                )));
            }
            WeightKind::Analytic | WeightKind::Frequency | WeightKind::Probability => {}
            WeightKind::Importance => {
                return Err(unsupported(
                    "OLS preparation received an unsupported weight kind",
                ));
            }
        }
    }
    Ok(())
}

fn loaded_weight(loaded: &LoadedOlsInput) -> Option<(WeightKind, &LoadedNumericColumn)> {
    match (
        &loaded.analytic_weight,
        &loaded.frequency_weight,
        &loaded.probability_weight,
    ) {
        (Some(column), None, None) => Some((WeightKind::Analytic, column)),
        (None, Some(column), None) => Some((WeightKind::Frequency, column)),
        (None, None, Some(column)) => Some((WeightKind::Probability, column)),
        _ => None,
    }
}

fn validate_loaded_column(name: &str, column: &LoadedNumericColumn, n: usize) -> Result<()> {
    if column.values.len() != n || column.missing.len() != n {
        return Err(invalid(format!(
            "loaded {name} values and missing mask must both have length {n}",
        )));
    }
    Ok(())
}

struct OlsPreparationPolicy;

impl NumericDomainPolicy for OlsPreparationPolicy {
    fn outcome(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }

    fn weight(&self, _row: usize, kind: WeightKind, value: f64) -> Result<DomainDecision> {
        match kind {
            WeightKind::Analytic if value == 0.0 => Ok(DomainDecision::Drop),
            WeightKind::Analytic if value < 0.0 => Err(invalid_data("analytic weight is negative")),
            WeightKind::Analytic => Ok(DomainDecision::Retain),
            WeightKind::Frequency
                if value > 0.0 && value.fract() == 0.0 && value <= f64::from(u32::MAX) =>
            {
                Ok(DomainDecision::Retain)
            }
            WeightKind::Frequency => Err(invalid_data(
                "frequency weight must be a positive integer representable as u32",
            )),
            WeightKind::Probability if value > 0.0 => Ok(DomainDecision::Retain),
            WeightKind::Probability => Err(invalid_data(
                "probability weight must be positive and finite",
            )),
            WeightKind::Importance => Err(unsupported(
                "OLS preparation received an unsupported weight kind",
            )),
        }
    }

    fn offset(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
        Err(invalid("OLS preparation received an offset"))
    }

    fn exposure(&self, _row: usize, _value: f64) -> Result<DomainDecision> {
        Err(invalid("OLS preparation received an exposure"))
    }
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
    use crate::metadata::{
        ComponentOrder, ExpandedTerm, SourceEntry, TermComponent, TermId, TermOrder,
    };

    fn request(source_count: usize, include_intercept: bool) -> OlsRequestV1 {
        let sources = (0..source_count)
            .map(|index| SourceEntry {
                id: SourceId::new(u32::try_from(index + 1).expect("small fixture")).expect("id"),
                canonical_name: format!("x{}", index + 1),
            })
            .collect::<Vec<_>>();
        let terms = sources
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let one_based = u32::try_from(index + 1).expect("small fixture");
                ExpandedTerm {
                    id: TermId::new(one_based).expect("term id"),
                    order: TermOrder::new(one_based).expect("term order"),
                    coefficient_slot: CoefficientSlot::new(one_based).expect("slot"),
                    raw_kind: RawTermKind::Variable,
                    whole_column_omitted: false,
                    canonical_token: format!("c.x{}", index + 1),
                    components: vec![TermComponent {
                        order: ComponentOrder::new(1).expect("component order"),
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
                }
            })
            .collect::<Vec<_>>();
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
        bindings.extend(
            sources
                .iter()
                .enumerate()
                .map(|(index, source)| OlsInputBinding {
                    stata_var_position: u32::try_from(index + 3).expect("small fixture"),
                    role: OlsInputRole::NumericSource(source.id),
                }),
        );
        OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            bindings,
            include_intercept,
        }
    }

    fn column(values: &[f64], missing: &[bool]) -> LoadedNumericColumn {
        LoadedNumericColumn {
            values: values.to_vec(),
            missing: missing.to_vec(),
        }
    }

    fn mixed_numeric_factor_request() -> OlsRequestV1 {
        let source = SourceEntry {
            id: SourceId::new(1).expect("source"),
            canonical_name: "g".to_owned(),
        };
        let mut terms = Vec::new();
        terms.push(ExpandedTerm {
            id: TermId::new(1).expect("term"),
            order: TermOrder::new(1).expect("order"),
            coefficient_slot: CoefficientSlot::new(1).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "c.g".to_owned(),
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
        });
        for (index, level, base, omitted, token, raw_operator) in [
            (2, 0.0, true, true, "0b.g", "0b"),
            (3, 1.0, false, false, "1.g", "1"),
            (4, 2.0, false, false, "2.g", "2"),
            (5, 3.0, false, false, "3.g", "3"),
        ] {
            terms.push(ExpandedTerm {
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
                    raw_operator: raw_operator.to_owned(),
                    time_series_state: TimeSeriesState::None,
                    raw_time_series_operator: String::new(),
                }],
            });
        }
        OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![source.clone()],
                terms,
            },
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
                    role: OlsInputRole::NumericSource(source.id),
                },
            ],
            include_intercept: true,
        }
    }

    #[test]
    fn exact_row_map_drop_precedence_bits_and_intercept_are_preserved() {
        let request = request(2, true);
        let negative_zero = -0.0_f64;
        let tiny = f64::from_bits(1);
        let loaded = LoadedOlsInput {
            candidate: vec![false, true, true, true, true, true],
            outcome: column(
                &[99.0, negative_zero, 2.0, 3.0, 4.0, tiny],
                &[false, false, true, false, false, false],
            ),
            numeric_sources: vec![
                (
                    SourceId::new(1).expect("id"),
                    column(
                        &[88.0, 10.0, 20.0, 30.0, 40.0, tiny],
                        &[false, false, true, true, false, false],
                    ),
                ),
                (
                    SourceId::new(2).expect("id"),
                    column(
                        &[77.0, 11.0, 21.0, 31.0, 41.0, negative_zero],
                        &[false, false, false, false, true, false],
                    ),
                ),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("prepare");
        assert_eq!(prepared.sample.retained_rows(), &[1, 5]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::ExcludedByIfIn,
                crate::DropReason::Retained,
                crate::DropReason::MissingOutcome,
                crate::DropReason::MissingRegressor,
                crate::DropReason::MissingRegressor,
                crate::DropReason::Retained,
            ]
        );
        assert_eq!(
            prepared.sample.outcome[0].to_bits(),
            negative_zero.to_bits()
        );
        assert_eq!(prepared.sample.outcome[1].to_bits(), tiny.to_bits());
        assert_eq!(
            prepared.sample.regressors[1][1].to_bits(),
            negative_zero.to_bits()
        );
        assert_eq!(prepared.model.columns.len(), 3);
        assert_eq!(
            prepared.model.columns[2],
            PreparedOlsColumn {
                coefficient_slot: CoefficientSlot::new(3).expect("slot"),
                kind: PreparedOlsColumnKind::Intercept,
            }
        );
    }

    #[test]
    fn numeric_factor_main_effects_preserve_slots_factorize_once_and_allow_dual_use() {
        let request = mixed_numeric_factor_request();
        let loaded = LoadedOlsInput {
            candidate: vec![false, true, true, true, true, true],
            outcome: column(
                &[99.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                &[false, false, false, true, false, false],
            ),
            numeric_sources: vec![(
                SourceId::new(1).expect("source"),
                column(
                    &[-7.5, -0.0, 1.0, 3.0, 2.0, 2.0],
                    &[false, false, false, false, true, false],
                ),
            )],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("factor preparation");

        assert_eq!(prepared.sample.retained_rows(), &[1, 2, 5]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::ExcludedByIfIn,
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::MissingOutcome,
                crate::DropReason::MissingRegressor,
                crate::DropReason::Retained,
            ]
        );
        assert_eq!(prepared.categorical_sources.len(), 1);
        assert_eq!(
            prepared.categorical_sources[0].encoded.levels,
            vec![0.0, 1.0, 2.0]
        );
        assert_eq!(prepared.categorical_sources[0].encoded.codes, vec![0, 1, 2]);
        assert_eq!(prepared.model.coefficients.len(), 6);
        assert_eq!(
            prepared.model.coefficients[1],
            PreparedOlsCoefficient {
                coefficient_slot: CoefficientSlot::new(2).expect("slot"),
                status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                generated_column_index: None,
            }
        );
        assert_eq!(prepared.model.columns.len(), 5);
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(0, row).expect("continuous"))
                .collect::<Vec<_>>(),
            vec![-0.0, 1.0, 2.0]
        );
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(1, row).expect("level one"))
                .collect::<Vec<_>>(),
            vec![0.0, 1.0, 0.0]
        );
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(2, row).expect("level two"))
                .collect::<Vec<_>>(),
            vec![0.0, 0.0, 1.0]
        );
        assert_eq!(
            (0..prepared.sample.n_active())
                .map(|row| prepared.column_value(3, row).expect("vanished level"))
                .collect::<Vec<_>>(),
            vec![0.0, 0.0, 0.0]
        );
        let PreparedOlsColumnKind::Categorical { level_code, .. } = prepared.model.columns[3].kind
        else {
            panic!("fourth generated column must be categorical");
        };
        assert_eq!(level_code, None);
    }

    #[test]
    fn categorical_domain_is_candidate_scoped_and_fails_before_sample_construction() {
        for invalid_value in [-1.0, 1.5] {
            let request = mixed_numeric_factor_request();
            let loaded = LoadedOlsInput {
                candidate: vec![false, true],
                outcome: column(&[1.0, 2.0], &[false, false]),
                numeric_sources: vec![(
                    SourceId::new(1).expect("source"),
                    column(&[invalid_value, 1.0], &[false, false]),
                )],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            };
            prepare_ols_data(&request, &loaded).expect("off-candidate invalid factor value");

            let mut candidate_invalid = loaded;
            candidate_invalid.candidate[0] = true;
            let error = prepare_ols_data(&request, &candidate_invalid)
                .expect_err("candidate invalid factor value");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn empty_and_all_dropped_samples_are_valid_preparation_results() {
        let empty = LoadedOlsInput {
            candidate: Vec::new(),
            outcome: column(&[], &[]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request(0, false), &empty).expect("empty");
        assert_eq!(prepared.sample.n_original, 0);
        assert_eq!(prepared.sample.n_active(), 0);
        assert!(prepared.model.columns.is_empty());

        let dropped = LoadedOlsInput {
            candidate: vec![true, false],
            outcome: column(&[1.0, 2.0], &[true, false]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request(0, true), &dropped).expect("all dropped");
        assert_eq!(prepared.sample.n_candidate, 1);
        assert_eq!(prepared.sample.n_active(), 0);
        assert_eq!(prepared.model.columns.len(), 1);
    }

    #[test]
    fn malformed_bindings_and_unsupported_terms_fail_closed() {
        let mut malformed = request(1, false);
        malformed.bindings.pop();
        assert_eq!(
            malformed
                .validate_and_compile()
                .expect_err("missing source binding")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut unsupported = request(1, false);
        unsupported.model.terms[0].raw_kind = RawTermKind::Factor;
        assert_eq!(
            unsupported
                .validate_and_compile()
                .expect_err("factor unsupported")
                .code,
            ErrorCode::Unsupported
        );
    }

    #[test]
    fn whole_column_flag_is_authoritative_for_omission() {
        let mut request = request(1, false);
        request.model.terms[0].components[0].omitted = MetadataFlag::True;
        assert_eq!(
            request
                .validate_and_compile()
                .expect("component flag is inapplicable")
                .columns
                .len(),
            1
        );
        request.model.terms[0].whole_column_omitted = true;
        let model = request
            .validate_and_compile()
            .expect("whole-column omission");
        assert!(model.columns.is_empty());
        assert_eq!(
            model.coefficients[0].status,
            PreparedOlsCoefficientStatus::SyntaxOmitted
        );
    }

    #[test]
    fn analytic_weights_preserve_positive_bits_and_lock_drop_semantics() {
        let mut request = request(1, false);
        request.bindings.push(OlsInputBinding {
            stata_var_position: 4,
            role: OlsInputRole::AnalyticWeight,
        });
        let tiny = f64::from_bits(1);
        let huge = 1.0e300_f64;
        let loaded = LoadedOlsInput {
            candidate: vec![false, true, true, true, true, true],
            outcome: column(&[9.0, 1.0, 2.0, 3.0, 4.0, 5.0], &[false; 6]),
            numeric_sources: vec![(
                SourceId::new(1).expect("id"),
                column(&[9.0, 10.0, 20.0, 30.0, 40.0, 50.0], &[false; 6]),
            )],
            analytic_weight: Some(column(
                &[-1.0, tiny, huge, 0.0, -0.0, 7.0],
                &[false, false, false, false, false, true],
            )),
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("analytic-weight prepare");
        assert_eq!(prepared.sample.retained_rows(), &[1, 2]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::ExcludedByIfIn,
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::InvalidOrMissingWeight,
                crate::DropReason::InvalidOrMissingWeight,
                crate::DropReason::InvalidOrMissingWeight,
            ]
        );
        let crate::data::PreparedWeights::Provided { kind, values } = &prepared.sample.weights
        else {
            panic!("analytic weights must remain present");
        };
        assert_eq!(*kind, WeightKind::Analytic);
        assert_eq!(values[0].to_bits(), tiny.to_bits());
        assert_eq!(values[1].to_bits(), huge.to_bits());
    }

    #[test]
    fn candidate_negative_weight_fails_but_off_candidate_negative_is_ignored() {
        let mut request = request(0, false);
        request.bindings.push(OlsInputBinding {
            stata_var_position: 3,
            role: OlsInputRole::AnalyticWeight,
        });
        let off_candidate = LoadedOlsInput {
            candidate: vec![false, true],
            outcome: column(&[1.0, 2.0], &[false; 2]),
            numeric_sources: Vec::new(),
            analytic_weight: Some(column(&[-1.0, 2.0], &[false; 2])),
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &off_candidate).expect("off-candidate weight");
        assert_eq!(prepared.sample.retained_rows(), &[1]);

        let mut candidate_negative = off_candidate;
        candidate_negative.candidate = vec![true, true];
        let error = prepare_ols_data(&request, &candidate_negative)
            .expect_err("candidate-negative analytic weight");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn analytic_weight_binding_and_loaded_column_must_match_exactly() {
        let mut duplicate = request(0, false);
        duplicate.bindings.extend([
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::AnalyticWeight,
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::AnalyticWeight,
            },
        ]);
        assert_eq!(
            duplicate
                .validate_and_compile()
                .expect_err("duplicate analytic weight")
                .code,
            ErrorCode::InvalidArgument
        );

        let unbound_loaded = LoadedOlsInput {
            candidate: vec![true],
            outcome: column(&[1.0], &[false]),
            numeric_sources: Vec::new(),
            analytic_weight: Some(column(&[1.0], &[false])),
            frequency_weight: None,
            probability_weight: None,
        };
        assert_eq!(
            prepare_ols_data(&request(0, false), &unbound_loaded)
                .expect_err("unbound loaded weight")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn frequency_weights_preserve_positive_integer_bits_and_candidate_scope() {
        let mut request = request(0, false);
        request.bindings.push(OlsInputBinding {
            stata_var_position: 3,
            role: OlsInputRole::FrequencyWeight,
        });
        let maximum = f64::from(u32::MAX);
        let loaded = LoadedOlsInput {
            candidate: vec![true, true, true, true, false],
            outcome: column(&[1.0, 2.0, 3.0, 4.0, 5.0], &[false; 5]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: Some(column(
                &[1.0, 2.0, maximum, f64::NAN, 1.5],
                &[false, false, false, true, false],
            )),
            probability_weight: None,
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("frequency-weight prepare");
        assert_eq!(prepared.sample.retained_rows(), &[0, 1, 2]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::InvalidOrMissingWeight,
                crate::DropReason::ExcludedByIfIn,
            ]
        );
        let crate::data::PreparedWeights::Provided { kind, values } = &prepared.sample.weights
        else {
            panic!("frequency weights must remain present");
        };
        assert_eq!(*kind, WeightKind::Frequency);
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].to_bits(), 1.0_f64.to_bits());
        assert_eq!(values[1].to_bits(), 2.0_f64.to_bits());
        assert_eq!(values[2].to_bits(), maximum.to_bits());
    }

    #[test]
    fn invalid_candidate_frequency_weights_fail_closed() {
        for invalid_weight in [
            0.0,
            -1.0,
            1.5,
            f64::from(u32::MAX) + 1.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            let mut request = request(0, false);
            request.bindings.push(OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::FrequencyWeight,
            });
            let loaded = LoadedOlsInput {
                candidate: vec![true],
                outcome: column(&[1.0], &[false]),
                numeric_sources: Vec::new(),
                analytic_weight: None,
                frequency_weight: Some(column(&[invalid_weight], &[false])),
                probability_weight: None,
            };
            let error = prepare_ols_data(&request, &loaded)
                .expect_err("invalid candidate frequency weight");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn frequency_weight_binding_is_unique_and_matches_loaded_input() {
        let mut duplicate = request(0, false);
        duplicate.bindings.extend([
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::FrequencyWeight,
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::FrequencyWeight,
            },
        ]);
        assert_eq!(
            duplicate
                .validate_and_compile()
                .expect_err("duplicate frequency-weight bindings")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut mixed = request(0, false);
        mixed.bindings.extend([
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::AnalyticWeight,
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::FrequencyWeight,
            },
        ]);
        assert_eq!(
            mixed
                .validate_and_compile()
                .expect_err("mixed weight bindings")
                .code,
            ErrorCode::InvalidArgument
        );

        let unbound_loaded = LoadedOlsInput {
            candidate: vec![true],
            outcome: column(&[1.0], &[false]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: Some(column(&[1.0], &[false])),
            probability_weight: None,
        };
        assert_eq!(
            prepare_ols_data(&request(0, false), &unbound_loaded)
                .expect_err("unbound loaded frequency weight")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn probability_weights_preserve_positive_bits_and_candidate_scope() {
        let mut request = request(0, false);
        request.bindings.push(OlsInputBinding {
            stata_var_position: 3,
            role: OlsInputRole::ProbabilityWeight,
        });
        let tiny = f64::from_bits(1);
        let huge = 1.0e300_f64;
        let loaded = LoadedOlsInput {
            candidate: vec![false, true, true, true],
            outcome: column(&[1.0, 2.0, 3.0, 4.0], &[false; 4]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: Some(column(
                &[-1.0, tiny, huge, f64::NAN],
                &[false, false, false, true],
            )),
        };
        let prepared = prepare_ols_data(&request, &loaded).expect("probability-weight prepare");
        assert_eq!(prepared.sample.retained_rows(), &[1, 2]);
        assert_eq!(
            prepared.sample.drop_reasons,
            vec![
                crate::DropReason::ExcludedByIfIn,
                crate::DropReason::Retained,
                crate::DropReason::Retained,
                crate::DropReason::InvalidOrMissingWeight,
            ]
        );
        let crate::data::PreparedWeights::Provided { kind, values } = &prepared.sample.weights
        else {
            panic!("probability weights must remain present");
        };
        assert_eq!(*kind, WeightKind::Probability);
        assert_eq!(values[0].to_bits(), tiny.to_bits());
        assert_eq!(values[1].to_bits(), huge.to_bits());
    }

    #[test]
    fn invalid_candidate_probability_weights_fail_closed() {
        for invalid_weight in [0.0, -0.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let mut request = request(0, false);
            request.bindings.push(OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::ProbabilityWeight,
            });
            let loaded = LoadedOlsInput {
                candidate: vec![true],
                outcome: column(&[1.0], &[false]),
                numeric_sources: Vec::new(),
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: Some(column(&[invalid_weight], &[false])),
            };
            let error = prepare_ols_data(&request, &loaded)
                .expect_err("invalid candidate probability weight");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn probability_weight_binding_is_unique_and_mutually_exclusive() {
        let mut duplicate = request(0, false);
        duplicate.bindings.extend([
            OlsInputBinding {
                stata_var_position: 3,
                role: OlsInputRole::ProbabilityWeight,
            },
            OlsInputBinding {
                stata_var_position: 4,
                role: OlsInputRole::ProbabilityWeight,
            },
        ]);
        assert_eq!(
            duplicate
                .validate_and_compile()
                .expect_err("duplicate probability-weight bindings")
                .code,
            ErrorCode::InvalidArgument
        );

        for other in [OlsInputRole::AnalyticWeight, OlsInputRole::FrequencyWeight] {
            let mut mixed = request(0, false);
            mixed.bindings.extend([
                OlsInputBinding {
                    stata_var_position: 3,
                    role: OlsInputRole::ProbabilityWeight,
                },
                OlsInputBinding {
                    stata_var_position: 4,
                    role: other,
                },
            ]);
            assert_eq!(
                mixed
                    .validate_and_compile()
                    .expect_err("mixed weight bindings")
                    .code,
                ErrorCode::InvalidArgument
            );
        }

        let unbound_loaded = LoadedOlsInput {
            candidate: vec![true],
            outcome: column(&[1.0], &[false]),
            numeric_sources: Vec::new(),
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: Some(column(&[1.0], &[false])),
        };
        assert_eq!(
            prepare_ols_data(&request(0, false), &unbound_loaded)
                .expect_err("unbound loaded probability weight")
                .code,
            ErrorCode::InvalidArgument
        );
    }
}
