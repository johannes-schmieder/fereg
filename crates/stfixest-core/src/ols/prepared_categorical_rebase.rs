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
    SourceId, TimeSeriesState,
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
    candidate_model: &PreparedOlsModel,
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
        let originally_base = term
            .components
            .iter()
            .any(|component| component.base == MetadataFlag::True);
        let explicitly_omitted = term.whole_column_omitted && !originally_base;
        let Some((kind, final_base_component)) = final_term_kind(
            term,
            candidate_model,
            sources,
            topology,
            explicitly_omitted,
        )?
        else {
            continue;
        };
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
            let kind = kind.ok_or_else(|| {
                invalid("active rebased categorical term has no executable column recipe")
            })?;
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
    candidate_model: &PreparedOlsModel,
    sources: &[PreparedCategoricalModelSource],
    topology: &FinalizedCategoricalTopology,
    explicitly_omitted: bool,
) -> Result<Option<(Option<PreparedOlsColumnKind>, bool)>> {
    match (term.raw_kind, term.components.as_slice()) {
        (RawTermKind::Variable, [component])
            if component.kind == ComponentKind::Continuous
                && component.level_state == LevelState::Absent
                && component.time_series_state == TimeSeriesState::None =>
        {
            let kind = if explicitly_omitted {
                None
            } else {
                Some(candidate_continuous_kind(
                    candidate_model,
                    term.coefficient_slot,
                    component.source,
                )?)
            };
            Ok(Some((kind, false)))
        }
        (RawTermKind::Factor, [component])
            if component.kind == ComponentKind::Categorical
                && component.level_state == LevelState::Present
                && component.time_series_state == TimeSeriesState::None =>
        {
            let categorical_source_index = categorical_source_index(sources, component.source)?;
            let source = &sources[categorical_source_index];
            let Some(level_code) = source.resolve_level(topology, component.level_value)? else {
                return Ok(None);
            };
            Ok(Some((
                Some(PreparedOlsColumnKind::Categorical {
                    source: component.source,
                    prepared_source_index: source.prepared_source_index(),
                    categorical_source_index,
                    level_bits: canonical_factor_value(component.level_value).to_bits(),
                    level_code: Some(level_code),
                }),
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
                let Some(level_code) = source.resolve_level(topology, component.level_value)? else {
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
                Some(PreparedOlsColumnKind::CategoricalInteraction {
                    components: prepared_components,
                }),
                final_base_component,
            )))
        }
        _ => Err(unsupported(
            "final categorical rebasing supports only continuous terms, numeric factor main effects, and numeric all-categorical interactions",
        )),
    }
}

fn candidate_continuous_kind(
    candidate_model: &PreparedOlsModel,
    coefficient_slot: CoefficientSlot,
    source: SourceId,
) -> Result<PreparedOlsColumnKind> {
    let coefficient_index = usize::try_from(coefficient_slot.get() - 1)
        .map_err(|_| invalid("candidate continuous coefficient slot does not fit usize"))?;
    let coefficient = candidate_model
        .coefficients
        .get(coefficient_index)
        .ok_or_else(|| invalid("candidate continuous coefficient slot is absent"))?;
    if coefficient.coefficient_slot != coefficient_slot
        || coefficient.status != PreparedOlsCoefficientStatus::Active
    {
        return Err(invalid(
            "candidate continuous coefficient identity or status is inconsistent",
        ));
    }
    let generated_column_index = coefficient.generated_column_index.ok_or_else(|| {
        invalid("candidate continuous coefficient has no generated column identity")
    })?;
    let column = candidate_model
        .columns
        .get(generated_column_index)
        .ok_or_else(|| invalid("candidate continuous generated column is absent"))?;
    if column.coefficient_slot != coefficient_slot {
        return Err(invalid(
            "candidate continuous generated column identity is inconsistent",
        ));
    }
    match &column.kind {
        PreparedOlsColumnKind::Continuous {
            source: candidate_source,
            prepared_source_index,
        } if *candidate_source == source => Ok(PreparedOlsColumnKind::Continuous {
            source,
            prepared_source_index: *prepared_source_index,
        }),
        _ => Err(invalid(
            "candidate continuous generated column recipe is inconsistent",
        )),
    }
}

fn categorical_source_index(
    sources: &[PreparedCategoricalModelSource],
    source: SourceId,
) -> Result<usize> {
    sources
        .iter()
        .position(|candidate| candidate.source() == source)
        .ok_or_else(|| invalid("rebased categorical model source is absent"))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(value: u32) -> CoefficientSlot {
        CoefficientSlot::new(value).expect("valid coefficient slot")
    }

    fn source(value: u32) -> SourceId {
        SourceId::new(value).expect("valid source id")
    }

    #[test]
    fn continuous_rebase_preserves_validated_prepared_source_index() {
        let source = source(17);
        let candidate = PreparedOlsModel {
            coefficients: vec![PreparedOlsCoefficient {
                coefficient_slot: slot(1),
                status: PreparedOlsCoefficientStatus::Active,
                generated_column_index: Some(0),
            }],
            columns: vec![PreparedOlsColumn {
                coefficient_slot: slot(1),
                kind: PreparedOlsColumnKind::Continuous {
                    source,
                    prepared_source_index: 3,
                },
            }],
        };
        assert_eq!(
            candidate_continuous_kind(&candidate, slot(1), source)
                .expect("continuous candidate recipe"),
            PreparedOlsColumnKind::Continuous {
                source,
                prepared_source_index: 3,
            }
        );
    }

    #[test]
    fn continuous_rebase_rejects_source_or_status_mismatch() {
        let source = source(17);
        let active = PreparedOlsModel {
            coefficients: vec![PreparedOlsCoefficient {
                coefficient_slot: slot(1),
                status: PreparedOlsCoefficientStatus::Active,
                generated_column_index: Some(0),
            }],
            columns: vec![PreparedOlsColumn {
                coefficient_slot: slot(1),
                kind: PreparedOlsColumnKind::Continuous {
                    source,
                    prepared_source_index: 3,
                },
            }],
        };
        assert!(candidate_continuous_kind(&active, slot(1), source(18)).is_err());

        let omitted = PreparedOlsModel {
            coefficients: vec![PreparedOlsCoefficient {
                coefficient_slot: slot(1),
                status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                generated_column_index: None,
            }],
            columns: Vec::new(),
        };
        assert!(candidate_continuous_kind(&omitted, slot(1), source).is_err());
    }
}
