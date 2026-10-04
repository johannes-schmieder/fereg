//! Final-sample code resolution on Stata's original coefficient axis.
//!
//! Stata expands factor-variable terms on the candidate sample, while absorbed
//! fixed-effect singleton deletion can remove complete factor levels. The
//! original slots and whole-column omission flags remain authoritative. Resolve
//! each original recipe against the compacted codes; a vanished active level
//! produces a zero column for the existing rank machinery. This never allocates
//! an observation-scale column or chooses a replacement base.

use crate::error::{ErrorCode, FeregError, Result};
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

/// Identity mapping on the original candidate-sample slots supplied by Stata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCategoricalOlsReportPlan {
    original_coefficient_count: usize,
    original_slots: Vec<CoefficientSlot>,
}

impl PreparedCategoricalOlsReportPlan {
    pub(crate) fn owned_buffer_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.original_slots)
    }

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
    validate_default_base_policy(graph)?;
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
        let kind = final_term_kind(term, candidate_model, sources, topology)?;
        let public_slot = term.coefficient_slot;
        original_slots.push(term.coefficient_slot);
        if term.whole_column_omitted {
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

    if original_slots.len() != original_coefficient_count
        || original_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| slot.get() as usize != index + 1)
    {
        return Err(invalid(
            "final categorical reporting slots do not preserve the original axis",
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

#[derive(Debug, Clone, Copy)]
struct CategoricalBasePolicy {
    source: SourceId,
    minimum_level_bits: u64,
    base_level_bits: Option<u64>,
}

/// The guarded metadata wire does not distinguish Stata's default minimum
/// base from an explicit `ib<minimum>.` request. Public syntax restrictions
/// belong to Stata. This native preflight retains the observable minimum-base
/// rule, allowing Stata's unbased single-level graph without choosing a base
/// or changing the original recipe's omission status.
fn validate_default_base_policy(graph: &CompiledTermGraph) -> Result<()> {
    let mut policies = Vec::new();
    policies
        .try_reserve_exact(graph.sources.len())
        .map_err(|_| allocation("unable to reserve categorical base-policy identities"))?;
    for term in &graph.terms {
        for component in &term.components {
            if component.kind != ComponentKind::Categorical
                || component.level_state != LevelState::Present
                || component.time_series_state != TimeSeriesState::None
            {
                continue;
            }
            let level_bits = canonical_factor_value(component.level_value).to_bits();
            let policy = if let Some(policy) = policies
                .iter_mut()
                .find(|policy: &&mut CategoricalBasePolicy| policy.source == component.source)
            {
                policy.minimum_level_bits =
                    minimum_level_bits(policy.minimum_level_bits, level_bits);
                policy
            } else {
                if policies.len() == policies.capacity() {
                    return Err(allocation(
                        "categorical base-policy identity capacity exhausted",
                    ));
                }
                policies.push(CategoricalBasePolicy {
                    source: component.source,
                    minimum_level_bits: level_bits,
                    base_level_bits: None,
                });
                policies
                    .last_mut()
                    .ok_or_else(|| invalid("categorical base-policy identity is absent"))?
            };
            if component.base == MetadataFlag::True {
                match policy.base_level_bits {
                    None => policy.base_level_bits = Some(level_bits),
                    Some(base) if base == level_bits => {}
                    Some(_) => {
                        return Err(unsupported(
                            "categorical final-sample rebasing requires exactly one base level per source",
                        ));
                    }
                }
            }
        }
    }
    for policy in policies {
        let single_unbased_level = policy.base_level_bits.is_none()
            && graph
                .terms
                .iter()
                .flat_map(|term| &term.components)
                .all(|component| {
                    component.source != policy.source
                        || component.kind != ComponentKind::Categorical
                        || component.level_state != LevelState::Present
                        || component.time_series_state != TimeSeriesState::None
                        || canonical_factor_value(component.level_value).to_bits()
                            == policy.minimum_level_bits
                });
        if !single_unbased_level && policy.base_level_bits != Some(policy.minimum_level_bits) {
            return Err(unsupported(
                "categorical final-sample rebasing requires Stata's default minimum base policy",
            ));
        }
    }
    Ok(())
}

fn minimum_level_bits(left: u64, right: u64) -> u64 {
    let left_value = f64::from_bits(left);
    let right_value = f64::from_bits(right);
    if right_value < left_value {
        right
    } else {
        left
    }
}

fn final_term_kind(
    term: &crate::metadata::ExpandedTerm,
    candidate_model: &PreparedOlsModel,
    sources: &[PreparedCategoricalModelSource],
    topology: &FinalizedCategoricalTopology,
) -> Result<Option<PreparedOlsColumnKind>> {
    match (term.raw_kind, term.components.as_slice()) {
        (RawTermKind::Variable, [component])
            if component.kind == ComponentKind::Continuous
                && component.level_state == LevelState::Absent
                && matches!(
                    component.time_series_state,
                    TimeSeriesState::None | TimeSeriesState::Materialized
                ) =>
        {
            let kind = if term.whole_column_omitted {
                None
            } else {
                Some(candidate_continuous_kind(
                    candidate_model,
                    term.coefficient_slot,
                    component.source,
                )?)
            };
            Ok(kind)
        }
        (RawTermKind::Factor, [component])
            if component.kind == ComponentKind::Categorical
                && component.level_state == LevelState::Present
                && component.time_series_state == TimeSeriesState::None =>
        {
            if term.whole_column_omitted {
                return Ok(None);
            }
            let categorical_source_index = categorical_source_index(sources, component.source)?;
            let source = &sources[categorical_source_index];
            let level_code = source.resolve_level(topology, component.level_value)?;
            Ok(Some(PreparedOlsColumnKind::Categorical {
                    source: component.source,
                    prepared_source_index: source.prepared_source_index(),
                    categorical_source_index,
                    level_bits: canonical_factor_value(component.level_value).to_bits(),
                    level_code,
                }))
        }
        (RawTermKind::Interaction | RawTermKind::Product, components)
            if components.len() >= 2
                && components.iter().all(|component| {
                    component.kind == ComponentKind::Categorical
                        && component.level_state == LevelState::Present
                        && component.time_series_state == TimeSeriesState::None
                }) =>
        {
            if term.whole_column_omitted {
                return Ok(None);
            }
            let mut prepared_components = Vec::new();
            prepared_components
                .try_reserve_exact(components.len())
                .map_err(|_| allocation("unable to reserve rebased interaction components"))?;
            for component in components {
                let categorical_source_index =
                    categorical_source_index(sources, component.source)?;
                let source = &sources[categorical_source_index];
                let level_code = source.resolve_level(topology, component.level_value)?;
                prepared_components.push(PreparedOlsCategoricalComponent {
                    source: component.source,
                    categorical_source_index,
                    level_bits: canonical_factor_value(component.level_value).to_bits(),
                    level_code,
                });
            }
            Ok(Some(PreparedOlsColumnKind::CategoricalInteraction {
                    components: prepared_components,
                }))
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
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn unsupported(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Unsupported, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{
        ComponentOrder, ExpandedTerm, SourceEntry, TermComponent, TermId, TermOrder,
    };

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
        let active_source = source(17);
        let active = PreparedOlsModel {
            coefficients: vec![PreparedOlsCoefficient {
                coefficient_slot: slot(1),
                status: PreparedOlsCoefficientStatus::Active,
                generated_column_index: Some(0),
            }],
            columns: vec![PreparedOlsColumn {
                coefficient_slot: slot(1),
                kind: PreparedOlsColumnKind::Continuous {
                    source: active_source,
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
        assert!(candidate_continuous_kind(&omitted, slot(1), active_source).is_err());
    }

    #[test]
    fn default_minimum_base_policy_is_required_natively() {
        let default = factor_graph(Some(1.0));
        validate_default_base_policy(&default).expect("default minimum base");

        let nonminimum = validate_default_base_policy(&factor_graph(Some(2.0)))
            .expect_err("nonminimum base must fail closed");
        assert_eq!(nonminimum.code, ErrorCode::Unsupported);
        assert!(nonminimum.message.contains("default minimum base policy"));

        let no_base = validate_default_base_policy(&factor_graph(None))
            .expect_err("no-base syntax must fail closed");
        assert_eq!(no_base.code, ErrorCode::Unsupported);
        assert!(no_base.message.contains("default minimum base policy"));
    }

    #[test]
    fn single_declared_level_does_not_require_or_infer_a_base() {
        let mut graph = factor_graph(None);
        graph.terms.truncate(1);
        validate_default_base_policy(&graph).expect("active single-level default graph");
        assert!(!graph.terms[0].whole_column_omitted);
        assert_eq!(graph.terms[0].components[0].base, MetadataFlag::False);

        // Repeated interaction occurrences are one declared level, even when
        // the actual source will contain other levels (a selected 2.f recipe).
        graph.terms[0].components[0].level_value = 2.0;
        let mut repeated = graph.terms[0].components[0].clone();
        repeated.order = ComponentOrder::new(2).expect("component order");
        graph.terms[0].components.push(repeated);
        graph.terms[0].raw_kind = RawTermKind::Interaction;
        validate_default_base_policy(&graph).expect("one distinct declared level");
        graph.terms[0].components[1].level_value = 3.0;
        assert_eq!(
            validate_default_base_policy(&graph)
                .expect_err("two unbased levels")
                .code,
            ErrorCode::Unsupported
        );

        let mut multiple = factor_graph(Some(1.0));
        multiple.terms[1].components[0].base = MetadataFlag::True;
        assert_eq!(
            validate_default_base_policy(&multiple)
                .expect_err("multiple bases")
                .code,
            ErrorCode::Unsupported
        );

        // Omission does not erase a level from the original declaration.
        let mut omitted_second_level = factor_graph(None);
        omitted_second_level.terms.truncate(2);
        omitted_second_level.terms[1].whole_column_omitted = true;
        omitted_second_level.terms[1].components[0].omitted = MetadataFlag::True;
        assert_eq!(
            validate_default_base_policy(&omitted_second_level)
                .expect_err("omitted distinct unbased level remains declared")
                .code,
            ErrorCode::Unsupported
        );
    }

    #[test]
    fn omitted_recipes_skip_final_sources_but_retain_grammar_validation() {
        let mut graph = factor_graph(Some(1.0));
        graph.terms.truncate(1);
        let model = PreparedOlsModel {
            coefficients: Vec::new(),
            columns: Vec::new(),
        };
        let topology =
            crate::fe::finalize_categorical_topology(&[], &[], &[]).expect("empty topology");
        assert_eq!(
            final_term_kind(&graph.terms[0], &model, &[], &topology)
                .expect("omitted factor needs no executable source"),
            None
        );
        let mut component = graph.terms[0].components[0].clone();
        component.order = ComponentOrder::new(2).expect("component order");
        graph.terms[0].components.push(component);
        graph.terms[0].raw_kind = RawTermKind::Interaction;
        assert_eq!(
            final_term_kind(&graph.terms[0], &model, &[], &topology)
                .expect("omitted interaction needs no executable sources"),
            None
        );
        graph.terms[0].components[0].time_series_state = TimeSeriesState::Materialized;
        assert_eq!(
            final_term_kind(&graph.terms[0], &model, &[], &topology)
                .expect_err("omitted categorical TS is still unsupported")
                .code,
            ErrorCode::Unsupported
        );
    }

    fn factor_graph(base_level: Option<f64>) -> CompiledTermGraph {
        let categorical = source(1);
        CompiledTermGraph {
            sources: vec![SourceEntry {
                id: categorical,
                canonical_name: "f".to_owned(),
            }],
            terms: [1.0, 2.0, 3.0]
                .into_iter()
                .enumerate()
                .map(|(index, level)| {
                    let one_based = u32::try_from(index + 1).expect("small term");
                    let base = base_level == Some(level);
                    ExpandedTerm {
                        id: TermId::new(one_based).expect("term id"),
                        order: TermOrder::new(one_based).expect("term order"),
                        coefficient_slot: slot(one_based),
                        raw_kind: RawTermKind::Factor,
                        whole_column_omitted: base,
                        canonical_token: format!("{level}.f"),
                        components: vec![TermComponent {
                            order: ComponentOrder::new(1).expect("component order"),
                            source: categorical,
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
                            raw_operator: level.to_string(),
                            time_series_state: TimeSeriesState::None,
                            raw_time_series_operator: String::new(),
                        }],
                    }
                })
                .collect(),
        }
    }
}
