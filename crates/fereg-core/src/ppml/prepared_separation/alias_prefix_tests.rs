use super::*;

fn owner_with_aliases(
    columns: &[Vec<f64>],
    g: &[f64],
    h: &[f64],
    y: &[f64],
    two_effects: bool,
) -> PreparedPpmlModelOwner {
    let (mut request, clusters, mut loaded, mut uses) = fixture();
    request.keep_singletons = true;
    if two_effects {
        let mut second = request.absorbed_terms[0].clone();
        second.id = AbsorbTermId::new(2).unwrap();
        second.categorical_sources = vec![SourceId::new(3).unwrap()];
        request.absorbed_terms.push(second);
        uses[2].fixed_effect = true;
    }
    loaded.candidate = vec![true; y.len()].into();
    loaded.outcome = column(y);
    loaded.numeric_sources[0].1 = column(&columns[0]);
    loaded.numeric_sources[1].1 = column(g);
    loaded.numeric_sources[2].1 = column(h);
    for (index, values) in columns.iter().enumerate().skip(1) {
        let id = SourceId::new(index as u32 + 3).unwrap();
        request
            .model
            .sources
            .push(source(id, &format!("alias{index}")));
        let mut term = request.model.terms[0].clone();
        term.id = TermId::new(index as u32 + 1).unwrap();
        term.order = TermOrder::new(index as u32 + 1).unwrap();
        term.coefficient_slot = CoefficientSlot::new(index as u32 + 1).unwrap();
        term.components[0].source = id;
        request.model.terms.push(term);
        request.bindings.push(OlsInputBinding {
            stata_var_position: index as u32 + 5,
            role: OlsInputRole::NumericSource(id),
        });
        loaded.numeric_sources.push((id, column(values)));
        uses.push(NumericSourceUse::ordinary());
    }
    PreparedPpmlModelOwner::new(
        request,
        clusters,
        loaded.clone(),
        prepare_numeric(&loaded, &uses),
    )
    .unwrap()
}

fn certify(generation: &PreparedPpmlModelGeneration, dual: &[f64]) -> Result<bool> {
    let ppml = detector_ppml(FeSolverKind::Cg);
    stable::certify_dual(
        generation,
        dual,
        ppml,
        rectifier_only(),
        PreparedOlsFitConfig {
            absorb: ppml.absorb,
            fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
            ..Default::default()
        },
        DetectorAdmission::default(),
    )
}

#[test]
fn exact_alias_prefix_uses_independently_proved_multi_fe_representative() {
    let g = vec![1., 1., 1., 1., 2., 2., 2., 2., 3., 3.];
    let h = vec![1., 1., 10., 10., 10., 10., 100., 100., 100., 100.];
    let x: Vec<_> = g.iter().zip(&h).map(|(a, b)| a + b).collect();
    let columns = vec![
        x.clone(),
        x.clone(),
        x.iter().map(|v| 2. * v + 3.).collect(),
        x.iter().map(|v| -0.5 * v + 7.).collect(),
    ];
    let y: Vec<_> = (0..10).map(|i| if i % 2 == 0 { 0. } else { 2. }).collect();
    let owner = owner_with_aliases(&columns, &g, &h, &y, true);
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    let generation = coordinator.view().prepared;
    // Neither the current column nor its future duplicates can bootstrap a
    // proof. Establish the first representative independently as g+h.
    assert!(!stable::exact_finite_duplicate(generation, 0, &[]).unwrap());
    assert!(stable::exact_integer_alias(generation, &mut x.clone(), &[], &[]).unwrap());
    for column in 1..columns.len() {
        assert!(stable::exact_finite_duplicate(generation, column, &[]).unwrap());
    }
    // Paired identical rows with outcomes(0,2) give an exact full-span dual.
    let dual: Vec<_> = (0..10).map(|i| if i % 2 == 0 { 1. } else { -1. }).collect();
    assert!(certify(generation, &dual).unwrap());
}

#[test]
fn exact_alias_prefix_cannot_bootstrap_circular_or_future_weak_aliases() {
    for power in [40, 48, 50] {
        let n = 48;
        let x: Vec<_> = (0..n).map(|i| (i % 3) as f64 - 1.).collect();
        let weak: Vec<_> = (0..n)
            .map(|i| x[i] + 2_f64.powi(-power) * (i % 2) as f64)
            .collect();
        let g: Vec<_> = (0..n).map(|i| (i / 12) as f64).collect();
        let h: Vec<_> = (0..n).map(|i| (i % 3) as f64).collect();
        let y: Vec<_> = (0..n).map(|i| if i % 2 == 0 { 1. } else { 0. }).collect();
        let owner = owner_with_aliases(&[x, weak.clone(), weak], &g, &h, &y, false);
        let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
        let generation = coordinator.view().prepared;
        // The first omitted contrast cannot borrow its later duplicate, so
        // the audit aborts before that duplicate could refer back to it.
        assert!(!stable::exact_finite_duplicate(generation, 1, &[0]).unwrap());
        let dual: Vec<_> = (0..n).map(|i| if i % 2 == 0 { -1. } else { 1. }).collect();
        match certify(generation, &dual) {
            Ok(false) => (),
            Err(error)
                if matches!(
                    error.code,
                    ErrorCode::NonConvergence | ErrorCode::InvalidData
                ) => {}
            other => panic!("unproved weak alias must refuse, got {other:?}"),
        }
    }
}
