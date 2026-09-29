use fereg_engine::{
    data::*,
    dof::{compute_absorbed_dof, DofPolicy, EffectIdentity},
    fe::{Design, Weights},
    spec::Estimator,
    threads::ThreadTeam,
    topology::prepare_implicit_constant,
    wls::Role,
    *,
};
mod support;
fn component(source: u32, categorical: bool, level: f64) -> TermComponent {
    TermComponent {
        order: ComponentOrder::new(1).unwrap(),
        source: SourceId::new(source).unwrap(),
        kind: if categorical {
            ComponentKind::Categorical
        } else {
            ComponentKind::Continuous
        },
        level_state: if categorical {
            LevelState::Present
        } else {
            LevelState::Absent
        },
        level_value: level,
        base: MetadataFlag::False,
        omitted: MetadataFlag::Unavailable,
        raw_operator: if categorical { "1" } else { "c" }.into(),
        time_series_state: TimeSeriesState::None,
        raw_time_series_operator: String::new(),
    }
}
fn term(id: u32, components: Vec<TermComponent>, omitted: bool) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(id).unwrap(),
        order: TermOrder::new(id).unwrap(),
        coefficient_slot: CoefficientSlot::new(id).unwrap(),
        raw_kind: if components.len() > 1 {
            RawTermKind::Interaction
        } else if components[0].kind == ComponentKind::Categorical {
            RawTermKind::Factor
        } else {
            RawTermKind::Variable
        },
        whole_column_omitted: omitted,
        canonical_token: format!("term{id}"),
        components,
    }
}
fn spec(terms: Vec<ExpandedTerm>) -> ModelSpec {
    ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=2)
                .map(|id| SourceEntry {
                    id: SourceId::new(id).unwrap(),
                    canonical_name: format!("s{id}"),
                })
                .collect(),
            terms,
        },
        absorbed_terms: vec![],
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    }
}
#[test]
fn whole_column_flags_own_omission_and_missing_levels_keep_slots() {
    let mut base = component(1, true, 0.);
    base.base = MetadataFlag::True;
    let mut ts = component(2, false, 0.);
    ts.time_series_state = TimeSeriesState::Materialized;
    ts.raw_time_series_operator = "L".into();
    let spec = spec(vec![
        term(1, vec![base.clone()], true),
        term(2, vec![base], false),
        term(3, vec![ts], false),
        term(4, vec![component(1, true, 9.)], false),
        term(5, vec![component(2, false, 0.)], true),
    ]);
    let compiled = CompiledColumns::new(&spec, Estimator::Ols).unwrap();
    assert_eq!(
        compiled
            .slots
            .iter()
            .map(|s| s.generated)
            .collect::<Vec<_>>(),
        vec![None, Some(0), Some(1), Some(2), None, Some(3)]
    );
    assert_eq!(compiled.slots[0].syntax, SyntaxStatus::Base);
    assert_eq!(compiled.slots[1].syntax, SyntaxStatus::Active);
    assert_eq!(compiled.slots[4].syntax, SyntaxStatus::ExplicitlyOmitted);
    assert_eq!(compiled.roles[3], Role::AbsorbedIntercept);
    let mut sample = support::prepared(&[], false, 1, 2);
    sample.regressors = vec![vec![0., 1., 0., 2.], vec![2., 3., 4., 5.]];
    let matrix = compiled.generate(&sample).unwrap();
    assert_eq!(matrix.column(0).unwrap(), vec![1., 0., 1., 0.]);
    assert_eq!(matrix.column(1).unwrap(), sample.regressors[1]);
    assert_eq!(matrix.column(2).unwrap(), vec![0.; 4]);
    assert_eq!(matrix.column(3).unwrap(), vec![1.; 4]);
    let hidden = CompiledColumns::new(
        &ModelSpec {
            include_intercept: false,
            ..spec
        },
        Estimator::Ols,
    )
    .unwrap();
    assert!(hidden.implicit_constant);
    assert!(hidden.structural_intercept);
    assert_eq!(hidden.constant_slot, None);
}
#[test]
fn categorical_interactions_use_literal_levels_without_a_second_code_bank() {
    let a = component(1, true, 1.);
    let mut b = component(2, true, 2.);
    b.order = ComponentOrder::new(2).unwrap();
    let spec = spec(vec![term(1, vec![a, b], false)]);
    let compiled = CompiledColumns::new(&spec, Estimator::Ols).unwrap();
    let mut sample = support::prepared(&[], false, 1, 2);
    sample.regressors = vec![vec![1., 1., 0., 0.], vec![2., 3., 2., 3.]];
    assert_eq!(
        compiled.generate(&sample).unwrap().column(0).unwrap(),
        vec![1., 0., 0., 0.]
    );
}
#[test]
fn slope_only_constant_policy_and_unsupported_grammar_are_explicit() {
    let mut model = spec(vec![]);
    model.absorbed_terms.push(AbsorbedTerm {
        id: AbsorbTermId::new(1).unwrap(),
        intercept: false,
        categorical_sources: vec![SourceId::new(1).unwrap()],
        slope_sources: vec![SourceId::new(2).unwrap()],
        label: "s1#c.s2".into(),
        saved_name: String::new(),
    });
    let ols = CompiledColumns::new(&model, Estimator::Ols).unwrap();
    assert!(!ols.structural_intercept);
    assert!(ols.slots.is_empty());
    let ppml = CompiledColumns::new(&model, Estimator::Ppml).unwrap();
    assert!(ppml.structural_intercept);
    assert_eq!(ppml.roles, vec![Role::Ordinary]);
    let mut x = component(1, false, 0.);
    let mut z = component(2, false, 0.);
    z.order = ComponentOrder::new(2).unwrap();
    assert!(CompiledColumns::new(
        &spec(vec![term(1, vec![x.clone(), z], true)]),
        Estimator::Ols
    )
    .is_err());
    x.time_series_state = TimeSeriesState::PendingMaterialization;
    x.raw_time_series_operator = "L".into();
    assert!(CompiledColumns::new(&spec(vec![term(1, vec![x], false)]), Estimator::Ols).is_err());
    assert!(CompiledColumns::new(
        &spec(vec![term(1, vec![component(1, true, 1.5)], false)]),
        Estimator::Ols
    )
    .is_err());
}
#[test]
fn virtual_constant_uses_shared_projection_and_dof_without_a_declared_effect() {
    let mut sample = support::prepared(&[], false, 2, 0);
    sample.outcome = (0..sample.n_active()).map(|i| i as f64).collect();
    let before = compute_absorbed_dof(&sample, true, DofPolicy::Pairwise, &[]).unwrap();
    let sample = prepare_implicit_constant(sample).unwrap();
    let after = compute_absorbed_dof(&sample, true, DofPolicy::Pairwise, &[]).unwrap();
    assert_eq!(before, after);
    assert_eq!(after.effects[0].identity, EffectIdentity::ImplicitNoAbsorb);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    assert_eq!(design.topology().effects()[0].term_id, None);
    let rhs =
        DenseMatrix::from_columns(&[sample.outcome.clone(), vec![1.; sample.n_active()]]).unwrap();
    let fit = design
        .direct_candidate(&rhs, &ThreadTeam::new(1).unwrap())
        .unwrap();
    assert!((fit.coefficients[(0, 0)] - 3.5).abs() < 1e-14);
    assert!(fit
        .residuals
        .column(1)
        .unwrap()
        .iter()
        .all(|v| v.abs() < 1e-14));
    let clusters = (0..sample.n_active())
        .map(|i| (i % 2) as u32)
        .collect::<Vec<_>>();
    let dof = compute_absorbed_dof(&sample, true, DofPolicy::Pairwise, &[&clusters]).unwrap();
    assert_eq!(dof.absorbed_rank, 1);
    assert_eq!(dof.nested_categories, 0);
    assert!(prepare_implicit_constant(sample).is_err());
    assert!(prepare_implicit_constant(support::prepared(&[3], false, 3, 0)).is_err());
}
