use super::*;

fn alias_fixture(
    two_effects: bool,
    multiple: f64,
) -> (OlsRequestV1, LoadedOlsInput, Vec<NumericSourceUse>) {
    let (mut request, _, mut loaded, mut uses) = fixture();
    request.keep_singletons = true;
    if two_effects {
        let mut second = request.absorbed_terms[0].clone();
        second.id = AbsorbTermId::new(2).unwrap();
        second.categorical_sources = vec![SourceId::new(3).unwrap()];
        request.absorbed_terms.push(second);
        uses[2] = NumericSourceUse::fixed_effect();
    } else {
        let mut second = request.model.terms[0].clone();
        second.id = TermId::new(2).unwrap();
        second.order = TermOrder::new(2).unwrap();
        second.coefficient_slot = CoefficientSlot::new(2).unwrap();
        second.components[0].source = SourceId::new(3).unwrap();
        request.model.terms.push(second);
        uses[2] = NumericSourceUse::ordinary();
    }
    let n = if two_effects { 24 } else { 32 };
    let (mut y, mut x, mut g, mut h) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for row in 0..n {
        let pair = row / 2;
        let group = (pair / 4) as f64;
        let other = (pair % 4) as f64;
        let value = if two_effects {
            group + other
        } else {
            [0.1, 1., 0.3, 2.][pair % 4]
        };
        y.push(if row % 2 == 0 { 0. } else { 2. });
        x.push(value);
        g.push(group);
        h.push(if two_effects { other } else { multiple * value });
    }
    loaded.candidate = vec![true; n].into();
    loaded.outcome = column(&y);
    loaded.numeric_sources[0].1 = column(&x);
    loaded.numeric_sources[1].1 = column(&g);
    loaded.numeric_sources[2].1 = column(&h);
    (request, loaded, uses)
}

fn check_alias_support(
    fixture: (OlsRequestV1, LoadedOlsInput, Vec<NumericSourceUse>),
    label: &str,
) {
    // Identical design rows occur in pairs with outcomes (0,2). Therefore
    // dual=(1,-1) per pair lies EXACTLY in the complete design's orthogonal
    // complement and is strictly positive on every zero. The finite MLE is
    // mu=1 everywhere: the pair scores cancel at beta=0 and zero FE values.
    let (ols, loaded, uses) = fixture;
    let n = loaded.outcome.values.len();
    let owner = PreparedPpmlModelOwner::new(
        ols.clone(),
        Vec::new(),
        loaded.clone(),
        prepare_numeric(&loaded, &uses),
    )
    .unwrap();
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    let ppml = detector_ppml(FeSolverKind::Cg);
    let config = rectifier_only();
    let linear = PreparedOlsFitConfig {
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    let dual: Vec<_> = (0..n)
        .map(|row| if row % 2 == 0 { 1. } else { -1. })
        .collect();
    let dual_certified =
        stable::certify_dual(coordinator.view().prepared, &dual, ppml, config, linear, 0).unwrap();
    let resolved = stable::resolve(
        coordinator.view().prepared,
        ppml,
        config,
        linear,
        &mut |_| Ok(()),
        0,
    );

    use crate::ppml::{
        fit_prepared_ppml, LoadedPpmlInput, PpmlMemoryConfig, PpmlSolverFamily, PpmlVceRequest,
        PreparedPpmlAdjustmentKind, PreparedPpmlOutputRequest, PreparedPpmlRequestV1,
    };
    let request = PreparedPpmlRequestV1 {
        bias: None,
        ols,
        clusters: Vec::new(),
        vce: PpmlVceRequest::Robust,
        ppml: PpmlConfig::default(),
        linear: PreparedOlsFitConfig::default(),
        memory: PpmlMemoryConfig::default(),
        memory_preflight_probe: None,
        allocation_snapshot_probe: None,
        phase_allocation_probe: None,
        linear_phase_allocation_probe: None,
        action_allocation_snapshot_probe: None,
        action_allocation_start: None,
        action_allocation_previous_baseline_bytes: None,
        separation: config,
        output: PreparedPpmlOutputRequest {
            residual: None,
            xb: false,
            absorbed: false,
            eta: false,
            mean: true,
            requested_observation_columns: 1,
            publication_matrix_cells: 512,
        },
        solver: PpmlSolverFamily::Auto,
        lsmr_preconditioner: crate::fe::LsmrPreconditioner::Auto,
        inject_fe_recovery_allocation_failure: false,
        inject_topology_sharing: false,
        inject_separation_cg: false,
    };
    let result = fit_prepared_ppml(
        request,
        LoadedPpmlInput {
            bias_units: None,
            ols: loaded,
            adjustment: None,
            adjustment_kind: PreparedPpmlAdjustmentKind::None,
        },
    )
    .unwrap_or_else(|error| panic!("case={label}, dual_certified={dual_certified}, resolution_error={:?}, full_fit_error={error:?}", resolved.as_ref().err()));
    assert!(dual_certified);
    assert!(matches!(resolved.unwrap(), stable::Resolution::Dual(_, _)));
    assert_eq!(result.sample().original_rows(), &(0..n).collect::<Vec<_>>());
    assert_eq!(result.separation().separated_count, 0);
    assert_eq!(result.output().mean().len(), n);
    for &mean in result.output().mean() {
        assert!((mean - 1.).abs() < 1e-10);
    }
}

#[test]
fn full_model_dual_accepts_alias_spanning_two_fixed_effects() {
    check_alias_support(alias_fixture(true, 1.), "balanced two-effect alias");
}

#[test]
fn full_model_dual_accepts_scaled_continuous_finite_aliases() {
    for multiple in [0.5, 2.] {
        check_alias_support(alias_fixture(false, multiple), "scaled finite alias");
    }
}

#[test]
fn exact_additive_alias_rejects_one_small_source_perturbation() {
    let (ols, loaded, uses) = alias_fixture(true, 1.);
    let owner = PreparedPpmlModelOwner::new(
        ols,
        Vec::new(),
        loaded.clone(),
        prepare_numeric(&loaded, &uses),
    )
    .unwrap();
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    let generation = coordinator.view().prepared;
    let original = loaded.numeric_sources[0].1.values.clone();
    assert!(stable::exact_integer_alias(generation, &mut original.clone(), &[], &[]).unwrap());
    // Same two-FE additive source, changed at exactly one stored row. The
    // binary64 perturbation is exact and must never become an alias proof.
    let mut integer_perturbed = original.clone();
    integer_perturbed[0] += 1.;
    assert!(!stable::exact_integer_alias(generation, &mut integer_perturbed, &[], &[]).unwrap());
    let mut perturbed = original;
    perturbed[0] += 2.0_f64.powi(-48);
    assert!(!stable::exact_integer_alias(generation, &mut perturbed, &[], &[]).unwrap());
}

#[test]
fn full_model_dual_accepts_additive_alias_on_unbalanced_connected_graph() {
    let (ols, mut loaded, uses) = alias_fixture(true, 1.);
    let edges = [(1., 1.), (1., 10.), (2., 10.), (2., 100.), (3., 100.)];
    let mut x = Vec::new();
    let mut g = Vec::new();
    let mut h = Vec::new();
    let mut y = Vec::new();
    for (a, b) in edges {
        for outcome in [0., 2.] {
            x.push(a + b);
            g.push(a);
            h.push(b);
            y.push(outcome);
        }
    }
    loaded.candidate = vec![true; y.len()].into();
    loaded.outcome = column(&y);
    loaded.numeric_sources[0].1 = column(&x);
    loaded.numeric_sources[1].1 = column(&g);
    loaded.numeric_sources[2].1 = column(&h);
    check_alias_support((ols, loaded, uses), "unbalanced two-effect additive alias");
}
