use std::collections::{BTreeMap, BTreeSet};

use crate::data::{
    prepare_numeric_sample, AdjustmentInput, DomainDecision, NumericColumnView,
    NumericDomainPolicy, NumericPreparationInput, PreparedNumericSample, WeightInput, WeightKind,
};
use crate::error::{ErrorCode, Result, StFixestError};
use crate::metadata::{
    CoefficientSlot, CompiledTermGraph, ComponentKind, LevelState, MetadataFlag, RawTermKind,
    SourceId, TimeSeriesState,
};

/// Role of one Stata plugin-varlist input in the developer OLS preparation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OlsInputRole {
    Candidate,
    Outcome,
    ContinuousSource(SourceId),
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

/// Complete raw input for the continuous, unweighted, no-FE preparation slice.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedOlsInput {
    pub candidate: Vec<bool>,
    pub outcome: LoadedNumericColumn,
    pub continuous_sources: Vec<(SourceId, LoadedNumericColumn)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsColumnKind {
    Intercept,
    Continuous {
        source: SourceId,
        prepared_source_index: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsColumn {
    pub coefficient_slot: CoefficientSlot,
    pub kind: PreparedOlsColumnKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOlsModel {
    pub columns: Vec<PreparedOlsColumn>,
}

#[derive(Debug, PartialEq)]
pub struct PreparedOlsData {
    pub model: PreparedOlsModel,
    pub sample: PreparedNumericSample,
}

impl OlsRequestV1 {
    /// Validate the entire request and compile only the currently supported
    /// continuous column recipes. This performs no observation reads.
    pub fn validate_and_compile(&self) -> Result<PreparedOlsModel> {
        self.model.validate_executable_metadata()?;
        validate_bindings(self)?;

        let mut columns = Vec::new();
        columns
            .try_reserve_exact(
                self.model
                    .terms
                    .len()
                    .checked_add(usize::from(self.include_intercept))
                    .ok_or_else(|| allocation("prepared OLS column count overflows"))?,
            )
            .map_err(|_| allocation("unable to reserve prepared OLS columns"))?;

        for term in &self.model.terms {
            if term.raw_kind != RawTermKind::Variable || term.components.len() != 1 {
                return Err(unsupported(
                    "prepare-ols-test supports only plain continuous model terms",
                ));
            }
            let component = &term.components[0];
            if component.kind != ComponentKind::Continuous
                || component.level_state != LevelState::Absent
                || component.base != MetadataFlag::False
                || component.time_series_state != TimeSeriesState::None
                || !matches!(component.raw_operator.as_str(), "" | "c" | "co")
            {
                return Err(unsupported(
                    "prepare-ols-test supports only non-time-series continuous sources",
                ));
            }

            // Never infer whole-column omission from component metadata. Stata's
            // term flag is the sole authority for whether this recipe is emitted.
            if !term.whole_column_omitted {
                let prepared_source_index = usize::try_from(component.source.get() - 1)
                    .map_err(|_| invalid("source id does not fit usize"))?;
                columns.push(PreparedOlsColumn {
                    coefficient_slot: term.coefficient_slot,
                    kind: PreparedOlsColumnKind::Continuous {
                        source: component.source,
                        prepared_source_index,
                    },
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
        }
        Ok(PreparedOlsModel { columns })
    }
}

/// Finish the Stata-independent boundary after the SPI layer has loaded raw inputs.
pub fn prepare_ols_data(
    request: &OlsRequestV1,
    loaded: &LoadedOlsInput,
) -> Result<PreparedOlsData> {
    let model = request.validate_and_compile()?;
    validate_loaded(request, loaded)?;

    let regressor_views = loaded
        .continuous_sources
        .iter()
        .map(|(_, column)| NumericColumnView::with_missing(&column.values, &column.missing))
        .collect::<Vec<_>>();
    let input = NumericPreparationInput {
        candidate: &loaded.candidate,
        outcome: NumericColumnView::with_missing(&loaded.outcome.values, &loaded.outcome.missing),
        regressors: &regressor_views,
        weights: WeightInput::None,
        adjustment: AdjustmentInput::None,
    };
    let sample = prepare_numeric_sample(&input, &OlsPreparationPolicy)?;
    Ok(PreparedOlsData { model, sample })
}

fn validate_bindings(request: &OlsRequestV1) -> Result<()> {
    let mut positions = BTreeSet::new();
    let mut candidate_count = 0_usize;
    let mut outcome_count = 0_usize;
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
            OlsInputRole::ContinuousSource(source) => {
                if sources.insert(source, binding.stata_var_position).is_some() {
                    return Err(invalid("continuous source is bound more than once"));
                }
            }
        }
    }
    if candidate_count != 1 || outcome_count != 1 {
        return Err(invalid(
            "OLS request requires exactly one candidate and one outcome binding",
        ));
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
    if loaded.continuous_sources.len() != request.model.sources.len() {
        return Err(invalid(
            "loaded continuous source count does not match the model registry",
        ));
    }
    for (index, (source, column)) in loaded.continuous_sources.iter().enumerate() {
        if *source != request.model.sources[index].id {
            return Err(invalid(
                "loaded continuous sources are not in model-registry order",
            ));
        }
        validate_loaded_column("continuous source", column, n)?;
    }
    Ok(())
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

    fn weight(&self, _row: usize, _kind: WeightKind, _value: f64) -> Result<DomainDecision> {
        Err(invalid("unweighted OLS preparation received a weight"))
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
                    role: OlsInputRole::ContinuousSource(source.id),
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
            continuous_sources: vec![
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
    fn empty_and_all_dropped_samples_are_valid_preparation_results() {
        let empty = LoadedOlsInput {
            candidate: Vec::new(),
            outcome: column(&[], &[]),
            continuous_sources: Vec::new(),
        };
        let prepared = prepare_ols_data(&request(0, false), &empty).expect("empty");
        assert_eq!(prepared.sample.n_original, 0);
        assert_eq!(prepared.sample.n_active(), 0);
        assert!(prepared.model.columns.is_empty());

        let dropped = LoadedOlsInput {
            candidate: vec![true, false],
            outcome: column(&[1.0, 2.0], &[true, false]),
            continuous_sources: Vec::new(),
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
        assert!(request
            .validate_and_compile()
            .expect("whole-column omission")
            .columns
            .is_empty());
    }
}
