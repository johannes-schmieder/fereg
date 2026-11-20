// Intersections of three effects and level-specific slopes. Every design row
// is distinct. Positive endpoints force a line in the two continuous values;
// the midpoint zero lies on it exactly, while a displaced zero can be strict.
// An opposite dyadic displacement forces this entire slope to bind.
use super::*;

fn coupled_model(local_x: bool, blockers: bool) -> (PreparedPpmlModelOwner, Vec<usize>) {
    let (mut request, _, mut loaded, mut uses) = fixture();
    let [x, g, h, k, z] = [1, 2, 3, 4, 5].map(|id| SourceId::new(id).unwrap());
    uses[2] = NumericSourceUse::fixed_effect();
    for (source_id, name) in [(k, "k"), (z, "z")] {
        request.model.sources.push(source(source_id, name));
        request.bindings.push(OlsInputBinding {
            stata_var_position: (request.bindings.len() + 1) as u32,
            role: OlsInputRole::NumericSource(source_id),
        });
        uses.push(NumericSourceUse::fixed_effect());
    }
    if local_x {
        request.model.terms.clear();
        uses[0] = NumericSourceUse::fixed_effect();
        request.absorbed_terms[0].slope_sources.push(x);
    }
    let mut effect = request.absorbed_terms[0].clone();
    effect.id = AbsorbTermId::new(2).unwrap();
    effect.categorical_sources = vec![h];
    effect.slope_sources.clear();
    effect.label = "h".into();
    request.absorbed_terms.push(effect.clone());
    effect.id = AbsorbTermId::new(3).unwrap();
    effect.categorical_sources = vec![k];
    effect.slope_sources = vec![z];
    effect.label = "k##c.z".into();
    request.absorbed_terms.push(effect);
    let mut rows = Vec::new();
    let mut expected = Vec::new();
    for j in 0..48 {
        let blocked = blockers && (!local_x || j % 3 == 2);
        for i in 0..8 {
            let (a, b, c) = (i % 2, i / 2 % 2, i / 4);
            let codes = [a, b, c].map(|v| (2 * j + v + 1) as f64);
            let center = (j % 13) as f64 / 4.;
            let fixed = (a + 4 * b + 2 * c) as f64;
            let slope = 1. + ((j + c) % 2) as f64;
            for (y, x, offset) in [
                (0., center, -1.),
                (2., center + 1., 0.),
                (0., center, 0.),
                (2., center - 1., 0.),
                (0., center, 2f64.powi(-40)),
            ] {
                if offset > 0. && !blocked {
                    continue;
                }
                if offset < 0. && !blocked {
                    expected.push(rows.len());
                }
                rows.push([
                    y,
                    x,
                    codes[0],
                    codes[1],
                    codes[2],
                    (offset - fixed - x) / slope,
                ]);
            }
        }
    }
    let col = |i: usize| column(&rows.iter().map(|row| row[i]).collect::<Vec<_>>());
    loaded.candidate = vec![true; rows.len()].into();
    loaded.outcome = col(0);
    loaded.numeric_sources = [x, g, h, k, z]
        .into_iter()
        .enumerate()
        .map(|(i, s)| (s, col(i + 1)))
        .collect();
    let numeric = prepare_numeric(&loaded, &uses);
    (
        PreparedPpmlModelOwner::new(request, vec![], loaded, numeric).unwrap(),
        expected,
    )
}

#[test]
fn sparse_slope_rays_separate_joint_coordinates_and_respect_near_blockers() {
    let config = SeparationConfig {
        methods: SeparationMethods {
            fixed_effects: false,
            simplex: false,
            iterated_rectifier: true,
            mu: false,
        },
        ..SeparationConfig::default()
    };
    for (local_x, blockers) in [(false, false), (false, true), (true, false), (true, true)] {
        let (owner, expected) = coupled_model(local_x, blockers);
        let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
        let generation = coordinator.view().prepared;
        let proposal =
            reconcile::sparse_slope_ray(generation, SeparationMethod::IteratedRectifier, config)
                .unwrap();
        if expected.is_empty() {
            assert!(proposal.is_none());
        } else {
            let proposal = proposal.expect("sparse original-coordinate witness");
            assert_eq!(proposal.dropped_original_rows, expected);
            let proof = proposal.exact_primal.as_ref().unwrap();
            let scores = proposal.direction.as_ref().unwrap();
            for (r, &value) in scores.iter().enumerate() {
                if expected.binary_search(&r).is_ok() {
                    assert!(value < 0.);
                } else {
                    assert_eq!(value, 0.);
                }
            }
            let budget = primal::ProofBudget::for_sparse(
                generation,
                SeparationMethod::IteratedRectifier,
                config,
            );
            assert!(primal::verify(generation, proof, &expected, budget, scores).unwrap());
        }
        let tiny = SeparationConfig {
            simplex_max_tableau_cells: 16,
            ..config
        };
        assert!(
            reconcile::sparse_slope_ray(generation, SeparationMethod::Simplex, tiny)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn sparse_slope_ray_requires_room_to_bind_and_replay_drop_rows() {
    let (owner, expected) = coupled_model(false, false);
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    let model = coordinator.view().prepared;
    let rows = model.outcome().len();
    let width = primal::RawRows::new(model, true).unwrap().width;
    // This cap fits the sparse echelon, but omits the coefficient and raw-view
    // storage live with the three row banks in the conservative replay bound.
    let run = |words| {
        let config = SeparationConfig {
            simplex_max_tableau_cells: words,
            ..SeparationConfig::default()
        };
        reconcile::sparse_slope_ray(model, SeparationMethod::Simplex, config).unwrap()
    };
    assert!(run(3 * rows).is_none());
    assert_eq!(
        run(4 * rows + width).unwrap().dropped_original_rows,
        expected
    );
}

#[test]
fn sparse_certificates_finish_without_admitting_an_unused_dense_fe_audit() {
    let mut budget = retry::tests::plan(None);
    budget.usable_bytes = Some(
        budget
            .required_peak_bytes
            .max(budget.selected_inventory().preparation_peak_bytes),
    );
    let (owner, expected) = coupled_model(false, false);
    let execution = PreparedPpmlSeparationExecution {
        adaptive_sparse: false,
        preliminary_plan: budget,
        linear: PreparedOlsFitConfig::default(),
        auto_projection: true,
    };
    let (outcome, ()) = fit_with_prepared_ppml_separation(
        owner,
        PpmlConfig::default(),
        rectifier_only(),
        None,
        execution,
        &mut |_| Ok(()),
        |_, _, _, _| unreachable!("Mu disabled"),
        |_, plan, _, _| {
            assert_eq!(
                plan.separation_projection_bytes,
                budget.separation_projection_bytes
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(outcome.separation.dropped_observations, expected);

    // On the retained sample each midpoint zero is the average of two
    // positive endpoint rows in every finite and absorbed coordinate. This
    // valid dual passes margin and raw moments. The numerical route must
    // still refuse its unfunded complete-span audit before allocation.
    let generation = &outcome.prepared;
    let dual: Vec<_> = generation
        .outcome()
        .iter()
        .map(|&y| if y == 0. { 1. } else { -0.5 })
        .collect();
    let memory = super::LiveMemoryPlan::new(budget);
    let error = stable::certify_dual(
        generation,
        &dual,
        PpmlConfig::default(),
        rectifier_only(),
        PreparedOlsFitConfig::default(),
        DetectorAdmission {
            span_plan: Some(&memory),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Allocation);
    assert!(error.message.contains("complete FE span certificate"));
    assert_eq!(memory.get(), budget);
}

/// Within each group the positive rows span the intercept and 64 slopes:
/// subtracting successive rows of min(i,j) gives an upper triangular matrix
/// with ones on its diagonal. Only the last slope is free on positives and
/// equals one on the group's single zero. Its coefficient -1 is an exact ray.
/// These repeated independent blocks require over 64M sparse entry touches,
/// while their local marginal costs and fill stay constant.
#[test]
fn productive_slope_elimination_crosses_the_production_work_window() {
    const SLOPES: usize = 64;
    const GROUPS: usize = 360;
    let group_size = SLOPES + 2;
    let n = GROUPS * group_size;
    let (mut request, _, mut loaded, _) = fixture();
    request.model.terms.clear();
    request.model.sources.truncate(2);
    request.bindings.truncate(4);
    loaded.numeric_sources.truncate(2);
    let variable = |f: &dyn Fn(usize) -> f64| column(&(0..n).map(f).collect::<Vec<_>>());
    loaded.candidate = vec![true; n].into();
    loaded.outcome = variable(&|r| f64::from(r % group_size != group_size - 1));
    loaded.numeric_sources[1].1 = variable(&|r| (r / group_size) as f64);
    let free = loaded.numeric_sources[0].0;
    loaded.numeric_sources[0].1 = variable(&|r| f64::from(r % group_size == group_size - 1));
    for j in 1..=SLOPES {
        let id = SourceId::new((j + 2) as u32).unwrap();
        request.model.sources.push(source(id, &format!("s{j}")));
        request.bindings.push(OlsInputBinding {
            stata_var_position: (request.bindings.len() + 1) as u32,
            role: OlsInputRole::NumericSource(id),
        });
        loaded
            .numeric_sources
            .push((id, variable(&|r| (r % group_size).min(j) as f64)));
        request.absorbed_terms[0].slope_sources.push(id);
    }
    request.absorbed_terms[0].slope_sources.push(free);
    let uses = vec![NumericSourceUse::fixed_effect(); loaded.numeric_sources.len()];
    let numeric = prepare_numeric(&loaded, &uses);
    let mut owner = PreparedPpmlModelOwner::new(request, vec![], loaded, numeric).unwrap();
    owner.bind_separation_plan(retry::tests::plan(Some(2 << 30)), true);
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    let model = coordinator.view().prepared;
    let method = SeparationMethod::Simplex;
    let config = SeparationConfig::default();
    // Both calls have the same >7M words, enough to hold this basis. Only
    // the adaptive work windows allow construction of the verified witness.
    assert!(primal::ProofBudget::for_sparse(model, method, config).words > 7_000_000);
    assert!(reconcile::sparse_slope_ray(model, method, config)
        .unwrap()
        .is_none());
    let adaptive = SeparationConfig {
        adaptive_sparse: true,
        ..config
    };
    let proposal = reconcile::sparse_slope_ray(model, method, adaptive)
        .unwrap()
        .expect("productive sparse elimination must continue");
    let expected: Vec<_> = (group_size - 1..n).step_by(group_size).collect();
    assert_eq!(proposal.dropped_original_rows, expected);
    let scores = proposal.direction.as_ref().unwrap();
    for (r, &z) in scores.iter().enumerate() {
        assert_eq!(
            z,
            if r % group_size == group_size - 1 {
                -1.
            } else {
                0.
            }
        );
    }
    certify_numerical_proposal(
        model,
        &proposal,
        PpmlConfig::default(),
        adaptive,
        PreparedOlsFitConfig::default(),
        false,
    )
    .unwrap();
}

#[test]
fn plateau_seed_replays_late_positive_equalities_and_dyadic_zero_blockers() {
    let n = 2_000;
    for (late_positive, last_zero) in [(0., 0.), (1., 0.), (0., -2f64.powi(-40))] {
        let (mut request, _, mut loaded, mut uses) = fixture();
        request.model.terms.clear();
        request.model.sources.truncate(2);
        request.bindings.truncate(4);
        loaded.numeric_sources.truncate(2);
        uses.truncate(2);
        let x = loaded.numeric_sources[0].0;
        request.absorbed_terms[0].slope_sources.push(x);
        uses.fill(NumericSourceUse::fixed_effect());
        let mut values = vec![0.; n];
        values[n - 3..].copy_from_slice(&[late_positive, 1., last_zero]);
        let mut outcome = vec![1.; n];
        outcome[n - 2..].fill(0.);
        loaded.candidate = vec![true; n].into();
        loaded.outcome = column(&outcome);
        loaded.numeric_sources[0].1 = column(&values);
        for (_, source) in &mut loaded.numeric_sources[1..] {
            *source = column(&vec![1.; n]);
        }
        let numeric = prepare_numeric(&loaded, &uses);
        let mut owner = PreparedPpmlModelOwner::new(request, vec![], loaded, numeric).unwrap();
        owner.bind_separation_plan(retry::tests::plan(Some(2 << 30)), true);
        let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
        let model = coordinator.view().prepared;
        let run = |adaptive_sparse| {
            let config = SeparationConfig {
                adaptive_sparse,
                ..rectifier_only()
            };
            let space = primal::SparseProofAdmission::new(
                model,
                SeparationMethod::IteratedRectifier,
                config,
            );
            primal::sparse_interpolant(model, space, 1_024).unwrap()
        };
        assert!(run(false).is_none());
        let proof = run(true);
        if late_positive == 0. && last_zero == 0. {
            let scores = primal::direction(model, &proof.expect("partial coefficient witness"))
                .unwrap()
                .unwrap();
            assert_eq!(scores[n - 2], -1.);
            assert!(scores[..n - 2].iter().all(|&z| z == 0.));
            assert_eq!(scores[n - 1], 0.);
        } else {
            assert!(
                proof.is_none(),
                "partial raw replay must reject every violated equality or sign"
            );
        }
    }
}
