//! Final-sample rebasing for ordinary categorical OLS terms.
//!
//! Stata expands factor-variable terms on the candidate sample, while absorbed
//! fixed-effect singleton deletion can remove complete factor levels. The
//! reference command then reports the factor graph expanded on the final sample.
//! This module rebuilds that small coefficient graph after authoritative sample
//! compaction. It never allocates an observation-scale column.

use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::FinalizedCategoricalTopology;
use crate::metadata::{
    CoefficientSlot, CompiledTermGraph, ComponentKind, LevelState, MetadataFlag, RawTermKind,
    TimeSeriesState,
};
use crate::prepared_categorical_data::PreparedCategoricalModelSource;

use super::{
    PreparedOlsCategoricalComponent, PreparedOlsCoefficient, PreparedOlsCoefficientStatus,
    PreparedOlsColumn, PreparedOlsColumnKind, PreparedOlsModel,
};

/// Compact mapping from final reported coefficient slots to the original
/// candidate-sample slots supplied by Stata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCategoricalOlsReportPlan {
    original_coefficient_count: usize,
    original_slots: Vec<CoefficientSlot>,
}

impl PreparedCategoricalOlsReportPlan {
    #[must_use]
    pub const fn original_coefficient_count(&self) -> usize {
        self.original_coefficient_count
    }

    #[must_use]
    pub fn original_slots(&self) -> &[CoefficientSlot] {
        &self.original_slots
    }

    #[must_use]
    pub fn public_coefficient_count(&self) -> usize {
        self.original_slots.len()
    }

    pub fn original_slot(&self, public_slot: CoefficientSlot) -> Result<CoefficientSlot> {
        let index = usize::try_from(public_slot.get() - 1)
            .map_err(|_| invalid("public categorical coefficient slot does not fit usize"))?;
        self.original_slots
            .get(index)
            .copied()
            .ok_or_else(|| invalid("public categorical coefficient slot is out of range"))
    }
}

pub(crate) struct RebasedCategoricalOlsModel {
    pub model: PreparedOlsModel,
    pub report_plan: PreparedCategoricalOlsReportPlan,
}

pub(crate) fn rebase_categorical_model(
    graph: &CompiledTermGraph,
    sources: &[PreparedCategoricalModelSource],
    topology: &FinalizedCategoricalTopology,
    report_intercept: bool,
) -> Result<RebasedCategoricalOlsModel> {
    let original_coefficient_count = graph
        .terms
        .len()
        .checked_add(usize::from(report_intercept))
        .ok_or_else(|| allocation("original categorical coefficient count overflows"))?;
    let mut coefficients = Vec::new();
    let mut columns = Vec::new();
    let mut original_slots = Vec::new();
    coefficients
        .try_reserve_exact(original_coefficient_count)
        .map_err(|_| allocation("unable to reserve rebased categorical coefficients"))?;
    columns
        .try_reserve_exact(original_coefficient_count)
        .map_err(|_| allocation("unable to reserve rebased categorical columns"))?;
    original_slots
        .try_reserve_exact(original_coefficient_count)
        .map_err(|_| allocation("unable to reserve categorical reporting slots"))?;

    for term in &graph.terms {
        let Some((kind, final_base_component)) =
            final_term_kind(term, sources, topology)?
        else {
            continue;
        };
        let originally_base = term
            .components
            .iter()
            .any(|component| component.base == MetadataFlag::True);
        let explicitly_omitted = term.whole_column_omitted && !originally_base;
        let syntax_omitted = explicitly_omitted || final_base_component;
        let public_slot = next_slot(coefficients.len())?;
        original_slots.push(term.coefficient_slot);
        if syntax_omitted {
            coefficients.push(PreparedOlsCoefficient {
                coefficient_slot: public_slot,
                status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                generated_column_index: None,
            });
        } else {
            let generated_column_index = columns.len();
            columns.push(PreparedOlsColumn {
                coefficient_slot: public_slot,
                kind,
            });
            coefficients.push(PreparedOlsCoefficient {
                coefficient_slot: public_slot,
                status: PreparedOlsCoefficientStatus::Active,
                generated_column_index: Some(generated_column_index),
            });
        }
    }

    if report_intercept {
        let public_slot = next_slot(coefficients.len())?;
        let original_slot = next_slot(graph.terms.len())?;
        original_slots.push(original_slot);
        let generated_column_index = columns.len();
        columns.push(PreparedOlsColumn {
            coefficient_slot: public_slot,
            kind: PreparedOlsColumnKind::Intercept,
        });
        coefficients.push(PreparedOlsCoefficient {
            coefficient_slot: public_slot,
            status: PreparedOlsCoefficientStatus::Active,
            generated_column_index: Some(generated_column_index),
        });
    }

    if original_slots
        .windows(2)
        .any(|pair| pair[0].get() >= pair[1].get())
        || original_slots
            .last()
            .is_some_and(|slot| slot.get() as usize > original_coefficient_count)
    {
        return Err(invalid(
            "rebased categorical reporting slots are not strictly ordered",
        ));
    }

    Ok(RebasedCategoricalOlsModel {
        model: PreparedOlsModel {
            coefficients,
            columns,
        },
        report_plan: PreparedCategoricalOlsReportPlan {
            original_coefficient_count,
            original_slots,
        },
    })
}

fn final_term_kind(
    term: &crate::metadata::ExpandedTerm,
    sources: &[PreparedCategoricalModelSource],
    topology: &FinalizedCategoricalTopology,
) -> Result<Option<(PreparedOlsColumnKind, bool)>> {
    match (term.raw_kind, term.components.as_slice()) {
        (RawTermKind::Variable, [component])
            if component.kind == ComponentKind::Continuous
                && component.level_state == LevelState::Absent
                && component.time_series_state == TimeSeriesState::None =>
        {
            Ok(Some((
                PreparedOlsColumnKind::Continuous {
                    source: component.source,
                    prepared_source_index: source_index(component.source)?,
                },
                false,
            )))
        }
        (RawTermKind::Factor, [component])
            if component.kind == ComponentKind::Categorical
                && component.level_state == LevelState::Present
                && component.time_series_state == TimeSeriesState::None =>
        {
            let categorical_source_index = categorical_source_index(sources, component.source)?;
            let source = &sources[categorical_source_index];
            let Some(level_code) =
                source.resolve_level(topology, component.level_value)?
            else {
                return Ok(None);
            };
            Ok(Some((
                PreparedOlsColumnKind::Categorical {
                    source: component.source,
                    prepared_source_index: source.prepared_source_index(),
                    categorical_source_index,
                    level_bits: canonical_factor_value(component.level_value).to_bits(),
                    level_code: Some(level_code),
                },
                level_code == 0,
            )))
        }
        (RawTermKind::Interaction | RawTermKind::Product, components)
            if components.len() >= 2
                && components.iter().all(|component| {
                    component.kind == ComponentKind::Categorical
                        && component.level_state == LevelState::Present
                        && component.time_series_state == TimeSeriesState::None
                }) =>
        {
            let mut prepared_components = Vec::new();
            prepared_components
                .try_reserve_exact(components.len())
                .map_err(|_| allocation("unable to reserve rebased interaction components"))?;
            let mut final_base_component = false;
            for component in components {
                let categorical_source_index =
                    categorical_source_index(sources, component.source)?;
                let source = &sources[categorical_source_index];
                let Some(level_code) =
                    source.resolve_level(topology, component.level_value)?
                else {
                    return Ok(None);
                };
                final_base_component |= level_code == 0;
                prepared_components.push(PreparedOlsCategoricalComponent {
                    source: component.source,
                    categorical_source_index,
                    level_bits: canonical_factor_value(component.level_value).to_bits(),
                    level_code: Some(level_code),
                });
            }
            Ok(Some((
                PreparedOlsColumnKind::CategoricalInteraction {
                    components: prepared_components,
                },
                final_base_component,
            )))
        }
        _ => Err(unsupported(
            "final categorical rebasing supports only continuous terms, numeric factor main effects, and numeric all-categorical interactions",
        )),
    }
}

fn categorical_source_index(
    sources: &[PreparedCategoricalModelSource],
    source: crate::metadata::SourceId,
) -> Result<usize> {
    sources
        .iter()
        .position(|candidate| candidate.source() == source)
        .ok_or_else(|| invalid("rebased categorical model source is absent"))
}

fn source_index(source: crate::metadata::SourceId) -> Result<usize> {
    usize::try_from(source.get() - 1)
        .map_err(|_| invalid("rebased continuous source id does not fit usize"))
}

fn next_slot(current_len: usize) -> Result<CoefficientSlot> {
    let one_based = current_len
        .checked_add(1)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| invalid("rebased categorical coefficient slot exceeds u32"))?;
    CoefficientSlot::new(one_based)
}

fn canonical_factor_value(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
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
