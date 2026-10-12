//! Final-sample code resolution on Stata's original coefficient axis.
//!
//! Stata expands factor-variable terms on the candidate sample, while absorbed
//! fixed-effect singleton deletion can remove complete factor levels. The
//! original slots and whole-column omission flags remain authoritative. Resolve
//! each original recipe against the compacted codes; a vanished active level
//! produces a zero column for the existing rank machinery. This never allocates
//! an observation-scale column or chooses a replacement base.

use crate::error::{allocation, invalid_argument as invalid, unsupported, Result};
use crate::fe::FinalizedCategoricalTopology;
use crate::metadata::{
    CoefficientSlot, CompiledTermGraph, ComponentKind, LevelState, MetadataFlag, RawTermKind,
    SourceId, TimeSeriesState,
};
use crate::ols::prepared_categorical_data::PreparedCategoricalModelSource;

use super::{PreparedOlsCoefficientStatus, PreparedOlsColumnKind, PreparedOlsModel};

/// Identity mapping on the original candidate-sample slots supplied by Stata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCategoricalOlsReportPlan {
    original_coefficient_count: usize,
}

impl PreparedCategoricalOlsReportPlan {
    pub(crate) const fn new(original_coefficient_count: usize) -> Self {
        Self {
            original_coefficient_count,
        }
    }

    #[must_use]
    pub const fn original_coefficient_count(&self) -> usize {
        self.original_coefficient_count
    }

    #[must_use]
    pub fn public_coefficient_count(&self) -> usize {
        self.original_coefficient_count
    }

    pub fn original_slot(&self, public_slot: CoefficientSlot) -> Result<CoefficientSlot> {
        let index = usize::try_from(public_slot.get() - 1)
            .map_err(|_| invalid("public categorical coefficient slot does not fit usize"))?;
        if index >= self.original_coefficient_count {
            return Err(invalid(
                "public categorical coefficient slot is out of range",
            ));
        }
        Ok(public_slot)
    }
}

#[cfg(test)]
struct RebasedCategoricalOlsModel {
    pub model: PreparedOlsModel,
    pub report_plan: PreparedCategoricalOlsReportPlan,
}

#[cfg(test)]
fn rebase_categorical_model(
    graph: &CompiledTermGraph,
    model: PreparedOlsModel,
    sources: &[PreparedCategoricalModelSource],
    topology: &FinalizedCategoricalTopology,
    report_intercept: bool,
) -> Result<RebasedCategoricalOlsModel> {
    let model = rebase_prepared_model(graph, model, sources, topology, report_intercept, true)?;
    let report_plan = PreparedCategoricalOlsReportPlan::new(model.coefficients.len());
    Ok(RebasedCategoricalOlsModel { model, report_plan })
}

pub(crate) fn rebase_prepared_model(
    graph: &CompiledTermGraph,
    mut model: PreparedOlsModel,
    sources: &[PreparedCategoricalModelSource],
    topology: &FinalizedCategoricalTopology,
    report_intercept: bool,
    categorical: bool,
) -> Result<PreparedOlsModel> {
    if categorical {
        validate_default_base_policy(graph)?;
    }

    let original_coefficient_count = graph
        .terms
        .len()
        .checked_add(usize::from(report_intercept))
        .ok_or_else(|| allocation("original categorical coefficient count overflows"))?;
    for term in &graph.terms {
        resolve_term_column_policy(term, &mut model, sources, topology, categorical)?;
    }
    // Synthetic availability marks slope sources as intercept partitions. Only
    // its provisional trailing constant may disappear for slope-only OLS.
    // Retain capacity: compaction must not allocate another model bank.
    if !report_intercept && model.coefficients.len() > original_coefficient_count {
        remove_provisional_intercept(&mut model, original_coefficient_count)?;
    }
    if model.coefficients.len() != original_coefficient_count
        || model
            .coefficients
            .iter()
            .enumerate()
            .any(|(index, coefficient)| coefficient.coefficient_slot.get() as usize != index + 1)
    {
        return Err(invalid(
            "final categorical reporting slots do not preserve the original axis",
        ));
    }
    Ok(model)
}

fn remove_provisional_intercept(model: &mut PreparedOlsModel, slots: usize) -> Result<()> {
    let coefficient = model
        .coefficients
        .last()
        .ok_or_else(|| invalid("provisional categorical intercept is absent"))?;
    let column = model
        .columns
        .last()
        .ok_or_else(|| invalid("provisional categorical intercept column is absent"))?;
    if model.coefficients.len() != slots + 1
        || coefficient.coefficient_slot.get() as usize != slots + 1
        || coefficient.status != PreparedOlsCoefficientStatus::Active
        || coefficient.generated_column_index != Some(model.columns.len() - 1)
        || column.coefficient_slot != coefficient.coefficient_slot
        || column.kind != PreparedOlsColumnKind::Intercept
    {
        return Err(invalid("provisional categorical intercept is inconsistent"));
    }
    model.coefficients.pop();
    model.columns.pop();
    Ok(())
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

fn validate_term_grammar(term: &crate::metadata::ExpandedTerm) -> Result<()> {
    match (term.raw_kind, term.components.as_slice()) {
        (RawTermKind::Variable, [component])
            if component.kind == ComponentKind::Continuous
                && component.level_state == LevelState::Absent
                && matches!(component.time_series_state,
                    TimeSeriesState::None | TimeSeriesState::Materialized) => Ok(()),
        (RawTermKind::Factor, [component])
            if component.kind == ComponentKind::Categorical
                && component.level_state == LevelState::Present
                && component.time_series_state == TimeSeriesState::None => Ok(()),
        (RawTermKind::Interaction | RawTermKind::Product, components)
            if components.len() >= 2 && components.iter().all(|component| {
                component.kind == ComponentKind::Categorical
                    && component.level_state == LevelState::Present
                    && component.time_series_state == TimeSeriesState::None
            }) => Ok(()),
        _ => Err(unsupported(
            "final categorical rebasing supports only continuous terms, numeric factor main effects, and numeric all-categorical interactions",
        )),
    }
}

#[cfg(test)]
fn resolve_term_column(
    term: &crate::metadata::ExpandedTerm,
    model: &mut PreparedOlsModel,
    sources: &[PreparedCategoricalModelSource],
    topology: &FinalizedCategoricalTopology,
) -> Result<()> {
    resolve_term_column_policy(term, model, sources, topology, true)
}

fn resolve_term_column_policy(
    term: &crate::metadata::ExpandedTerm,
    model: &mut PreparedOlsModel,
    sources: &[PreparedCategoricalModelSource],
    topology: &FinalizedCategoricalTopology,
    categorical: bool,
) -> Result<()> {
    if categorical {
        validate_term_grammar(term)?;
    }

    if term.whole_column_omitted {
        return Ok(());
    }
    let coefficient_index = usize::try_from(term.coefficient_slot.get() - 1)
        .map_err(|_| invalid("candidate continuous coefficient slot does not fit usize"))?;
    let coefficient = model
        .coefficients
        .get(coefficient_index)
        .ok_or_else(|| invalid("candidate continuous coefficient slot is absent"))?;
    if coefficient.coefficient_slot != term.coefficient_slot
        || coefficient.status != PreparedOlsCoefficientStatus::Active
    {
        return Err(invalid(
            "candidate continuous coefficient identity or status is inconsistent",
        ));
    }
    let column_index = coefficient.generated_column_index.ok_or_else(|| {
        invalid("candidate continuous coefficient has no generated column identity")
    })?;
    let column = model
        .columns
        .get_mut(column_index)
        .ok_or_else(|| invalid("candidate continuous generated column is absent"))?;
    if column.coefficient_slot != term.coefficient_slot {
        return Err(invalid(
            "candidate continuous generated column identity is inconsistent",
        ));
    }
    match &mut column.kind {
        PreparedOlsColumnKind::Continuous { source, .. }
            if term.components.len() == 1 && *source == term.components[0].source => {}
        PreparedOlsColumnKind::Categorical {
            source,
            prepared_source_index,
            categorical_source_index: index,
            level_bits,
            level_code,
        } if term.components.len() == 1
            && *source == term.components[0].source
            && *level_bits == canonical_factor_value(term.components[0].level_value).to_bits() =>
        {
            *index = categorical_source_index(sources, *source)?;
            *prepared_source_index = sources[*index].prepared_source_index();
            *level_code = sources[*index].resolve_level(topology, f64::from_bits(*level_bits))?;
        }
        PreparedOlsColumnKind::CategoricalInteraction { components }
            if components.len() == term.components.len() =>
        {
            for (component, original) in components.iter_mut().zip(&term.components) {
                if component.source != original.source
                    || component.level_bits
                        != canonical_factor_value(original.level_value).to_bits()
                {
                    return Err(invalid(
                        "candidate categorical interaction recipe is inconsistent",
                    ));
                }
                component.categorical_source_index =
                    categorical_source_index(sources, component.source)?;
                component.level_code = sources[component.categorical_source_index]
                    .resolve_level(topology, f64::from_bits(component.level_bits))?;
            }
        }
        _ => {
            return Err(invalid(
                "candidate continuous generated column recipe is inconsistent",
            ))
        }
    }
    Ok(())
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

fn canonical_factor_value(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::metadata::{
        ComponentOrder, ExpandedTerm, SourceEntry, TermComponent, TermId, TermOrder,
    };
    use crate::ols::{PreparedOlsCoefficient, PreparedOlsColumn};

    fn slot(value: u32) -> CoefficientSlot {
        CoefficientSlot::new(value).expect("valid coefficient slot")
    }

    fn source(value: u32) -> SourceId {
        SourceId::new(value).expect("valid source id")
    }

    #[test]
    fn continuous_rebase_preserves_validated_prepared_source_index() {
        let source = source(17);
        let mut candidate = PreparedOlsModel {
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
        candidate.coefficients.reserve(11);
        candidate.columns.reserve(13);
        let coefficients = (
            candidate.coefficients.as_ptr(),
            candidate.coefficients.capacity(),
        );
        let columns = (candidate.columns.as_ptr(), candidate.columns.capacity());
        let topology = crate::fe::finalize_categorical_topology(&[], &[], &[]).unwrap();
        let result =
            rebase_categorical_model(&continuous_graph(source), candidate, &[], &topology, false)
                .expect("continuous candidate recipe");
        assert_eq!(
            result.model.columns[0].kind,
            PreparedOlsColumnKind::Continuous {
                source,
                prepared_source_index: 3,
            }
        );
        assert_eq!(
            (
                result.model.coefficients.as_ptr(),
                result.model.coefficients.capacity()
            ),
            coefficients
        );
        assert_eq!(
            (
                result.model.columns.as_ptr(),
                result.model.columns.capacity()
            ),
            columns
        );
        assert_eq!(result.report_plan.original_slot(slot(1)).unwrap(), slot(1));
        assert!(result.report_plan.original_slot(slot(2)).is_err());
    }

    #[test]
    fn continuous_rebase_rejects_source_or_status_mismatch() {
        let active_source = source(17);
        let mut active = PreparedOlsModel {
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
        let topology = crate::fe::finalize_categorical_topology(&[], &[], &[]).unwrap();
        assert!(resolve_term_column(
            &continuous_graph(source(18)).terms[0],
            &mut active,
            &[],
            &topology
        )
        .is_err());

        let mut omitted = PreparedOlsModel {
            coefficients: vec![PreparedOlsCoefficient {
                coefficient_slot: slot(1),
                status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                generated_column_index: None,
            }],
            columns: Vec::new(),
        };
        assert!(resolve_term_column(
            &continuous_graph(active_source).terms[0],
            &mut omitted,
            &[],
            &topology
        )
        .is_err());
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
        let mut model = PreparedOlsModel {
            coefficients: Vec::new(),
            columns: Vec::new(),
        };
        let topology =
            crate::fe::finalize_categorical_topology(&[], &[], &[]).expect("empty topology");
        resolve_term_column(&graph.terms[0], &mut model, &[], &topology)
            .expect("omitted factor needs no executable source");
        let mut component = graph.terms[0].components[0].clone();
        component.order = ComponentOrder::new(2).expect("component order");
        graph.terms[0].components.push(component);
        graph.terms[0].raw_kind = RawTermKind::Interaction;
        resolve_term_column(&graph.terms[0], &mut model, &[], &topology)
            .expect("omitted interaction needs no executable sources");
        graph.terms[0].components[0].time_series_state = TimeSeriesState::Materialized;
        assert_eq!(
            resolve_term_column(&graph.terms[0], &mut model, &[], &topology)
                .expect_err("omitted categorical TS is still unsupported")
                .code,
            ErrorCode::Unsupported
        );
    }

    #[test]
    fn provisional_constant_removal_preserves_buffers_and_rejects_another_tail() {
        let graph = continuous_graph(source(1));
        let request = crate::ols::OlsRequestV1 {
            model: graph.clone(),
            absorbed_terms: Vec::new(),
            bindings: vec![
                crate::ols::OlsInputBinding {
                    stata_var_position: 1,
                    role: crate::ols::OlsInputRole::Candidate,
                },
                crate::ols::OlsInputBinding {
                    stata_var_position: 2,
                    role: crate::ols::OlsInputRole::Outcome,
                },
                crate::ols::OlsInputBinding {
                    stata_var_position: 3,
                    role: crate::ols::OlsInputRole::NumericSource(source(1)),
                },
            ],
            include_intercept: true,
            keep_singletons: true,
        };
        let model = request.validate_and_compile().unwrap();
        let coefficients = (model.coefficients.as_ptr(), model.coefficients.capacity());
        let columns = (model.columns.as_ptr(), model.columns.capacity());
        let mut inconsistent = model.clone();
        inconsistent.columns.last_mut().unwrap().kind = inconsistent.columns[0].kind.clone();
        assert!(remove_provisional_intercept(&mut inconsistent, 1).is_err());
        assert_eq!(inconsistent.coefficients.len(), 2);
        let topology = crate::fe::finalize_categorical_topology(&[], &[], &[]).unwrap();
        let resolved = rebase_categorical_model(&graph, model, &[], &topology, false).unwrap();
        assert_eq!(resolved.model.coefficients.len(), 1);
        assert_eq!(resolved.model.columns.len(), 1);
        assert_eq!(
            (
                resolved.model.coefficients.as_ptr(),
                resolved.model.coefficients.capacity()
            ),
            coefficients
        );
        assert_eq!(
            (
                resolved.model.columns.as_ptr(),
                resolved.model.columns.capacity()
            ),
            columns
        );
    }

    #[test]
    fn interaction_finalization_transfers_the_compiled_component_bank() {
        use crate::ols::{
            LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole, OlsRequestV1,
        };
        let mut graph = factor_graph(Some(1.));
        graph.terms[1].raw_kind = RawTermKind::Interaction;
        graph.terms[1].canonical_token = "2.f#2.f".to_owned();
        let mut second = graph.terms[1].components[0].clone();
        second.order = ComponentOrder::new(2).unwrap();
        graph.terms[1].components.push(second);
        let request = OlsRequestV1 {
            model: graph,
            absorbed_terms: Vec::new(),
            include_intercept: true,
            keep_singletons: true,
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
                    role: OlsInputRole::NumericSource(source(1)),
                },
            ],
        };
        let loaded = LoadedOlsInput {
            candidate: vec![true; 4].into(),
            outcome: LoadedNumericColumn {
                values: vec![1.; 4],
                missing: vec![false; 4],
            },
            numeric_sources: vec![(
                source(1),
                LoadedNumericColumn {
                    values: vec![1., 2., 3., 2.],
                    missing: vec![false; 4],
                },
            )],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = crate::ols::prepared_categorical_data::prepare_ppml_categorical_ols_data_with_load_hint(
            &request,
            &loaded,
            &[], None, 0)
        .unwrap();
        let model = request.validate_and_compile().unwrap();
        let bank = match &model.columns[0].kind {
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                (components.as_ptr(), components.capacity())
            }
            _ => panic!("interaction recipe"),
        };
        let resolved = rebase_categorical_model(
            &request.model,
            model,
            prepared.categorical_sources(),
            prepared.fe_topology(),
            true,
        )
        .unwrap();
        match &resolved.model.columns[0].kind {
            PreparedOlsColumnKind::CategoricalInteraction { components } => {
                assert_eq!((components.as_ptr(), components.capacity()), bank);
                assert!(components.iter().all(|c| c.level_code == Some(1)));
            }
            _ => panic!("interaction recipe"),
        }
        assert_eq!(resolved.model, *prepared.model());
    }

    fn continuous_graph(id: SourceId) -> CompiledTermGraph {
        let mut graph = factor_graph(Some(1.));
        graph.sources[0].id = id;
        graph.terms.truncate(1);
        let term = &mut graph.terms[0];
        term.raw_kind = RawTermKind::Variable;
        term.whole_column_omitted = false;
        term.canonical_token = "x".to_owned();
        let component = &mut term.components[0];
        component.source = id;
        component.kind = ComponentKind::Continuous;
        component.level_state = LevelState::Absent;
        component.level_value = 0.;
        component.base = MetadataFlag::False;
        component.omitted = MetadataFlag::False;
        component.raw_operator = "c".to_owned();
        graph
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
