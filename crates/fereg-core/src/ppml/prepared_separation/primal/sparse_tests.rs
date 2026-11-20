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
