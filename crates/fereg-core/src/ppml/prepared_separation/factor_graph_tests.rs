use super::*;

#[derive(Clone, Copy)]
enum Encoding {
    Ordinary { base: usize, partial_right: bool },
    Mixed,
    PureAbsorbed,
}

fn model(rows: &[[f64; 4]], levels: usize, encoding: Encoding) -> PreparedPpmlModelOwner {
    let (mut request, _, mut loaded, mut uses) = fixture();
    let finite_template = request.model.terms[0].clone();
    let absorbed_template = request.absorbed_terms[0].clone();
    request.absorbed_terms.clear();
    request.keep_singletons = true;
    let mut clusters = vec![];
    if matches!(encoding, Encoding::PureAbsorbed) {
        request.model.terms.clear();
        uses[0] = NumericSourceUse {
            ordinary: false,
            fixed_effect: false,
            cluster: true,
        };
        clusters.push(OlsClusterTermV1 {
            label: "x".into(),
            sources: vec![SourceId::new(1).unwrap()],
        });
    }
    for side in 0..2 {
        let source = SourceId::new(side + 2).unwrap();
        let absorbed = matches!(encoding, Encoding::PureAbsorbed)
            || (side == 0 && matches!(encoding, Encoding::Mixed));
        uses[side as usize + 1] = if absorbed {
            let mut term = absorbed_template.clone();
            term.id = AbsorbTermId::new(side + 1).unwrap();
            term.categorical_sources = vec![source];
            term.label = ["g", "h"][side as usize].into();
            request.absorbed_terms.push(term);
            NumericSourceUse::fixed_effect()
        } else {
            let base = match encoding {
                Encoding::Ordinary { base, .. } => base,
                _ => 0,
            };
            for level in 0..levels {
                if level == base
                    || (side == 1
                        && level == 1
                        && matches!(
                            encoding,
                            Encoding::Ordinary {
                                partial_right: true,
                                ..
                            }
                        ))
                {
                    continue;
                }
                let id = request.model.terms.len() as u32 + 1;
                let mut term = finite_template.clone();
                term.id = TermId::new(id).unwrap();
                term.order = TermOrder::new(id).unwrap();
                term.coefficient_slot = CoefficientSlot::new(id).unwrap();
                term.raw_kind = RawTermKind::Factor;
                term.canonical_token = format!("{}.{}", level, ["g", "h"][side as usize]);
                let c = &mut term.components[0];
                c.source = source;
                c.kind = ComponentKind::Categorical;
                c.level_state = LevelState::Present;
                c.level_value = level as f64;
                c.omitted = MetadataFlag::False;
                c.raw_operator = level.to_string();
                request.model.terms.push(term);
            }
            NumericSourceUse::ordinary()
        };
    }
    loaded.candidate = vec![true; rows.len()].into();
    let values = |i: usize| column(&rows.iter().map(|r| r[i]).collect::<Vec<_>>());
    loaded.outcome = values(0);
    for i in 0..3 {
        loaded.numeric_sources[i].1 = values(i + 1);
    }
    let numeric = prepare_numeric(&loaded, &uses);
    PreparedPpmlModelOwner::new(request, clusters, loaded, numeric).unwrap()
}

#[test]
fn indicator_rays_replay_across_bases_absorption_and_distinct_tags() {
    // One signed four-corner row identity per block and tag. Dropping the
    // reverse zero edge in the first block makes its forward edges strict
    // by the witness 1[left=1]-1[right=1], whatever the factor bases.
    let blocks = 104;
    let mut rows = Vec::new();
    for block in (0..blocks).rev() {
        for tag in 0..64 {
            for corner in [3, 2, 1, 0] {
                if block == 0 && corner == 2 {
                    continue;
                }
                rows.push([
                    f64::from(corner == 0 || corner == 3),
                    tag as f64,
                    (2 * block + corner / 2) as f64,
                    (2 * block + corner % 2) as f64,
                ]);
            }
        }
    }
    let expected: Vec<_> = (0..rows.len())
        .filter(|&i| rows[i][2] == 0. && rows[i][3] == 1.)
        .collect();
    assert_eq!(expected.len(), 64);
    let config = SeparationConfig::default();
    for encoding in [
        Encoding::Ordinary {
            base: 0,
            partial_right: false,
        },
        Encoding::Ordinary {
            base: 1,
            partial_right: false,
        },
        Encoding::Mixed,
    ] {
        let coordinator =
            PreparedSeparationCoordinator::new(model(&rows, 2 * blocks, encoding)).unwrap();
        let generation = coordinator.view().prepared;
        let proposal = reconcile::factor_pair(generation, SeparationMethod::Simplex, config)
            .unwrap()
            .unwrap();
        assert_eq!(proposal.dropped_original_rows, expected);
        let proof = proposal.exact_primal.as_ref().unwrap();
        let z = proposal.direction.as_ref().unwrap();
        let budget = primal::ProofBudget::new(generation, SeparationMethod::Simplex, config);
        assert!(primal::verify(generation, proof, &expected, budget, z).unwrap());
        // Integral scores match the model span even below machine epsilon.
        let strict = SeparationConfig {
            projection_tolerance: f64::EPSILON / 8.,
            ..config
        };
        let validated = validate_numerical_proposal(
            generation,
            proposal.clone(),
            PpmlConfig::default(),
            strict,
            PreparedOlsFitConfig::default(),
        )
        .unwrap();
        assert_eq!(validated.certificate.span_residual_relative, 0.);
        let mut forged = z.clone();
        forged[0] = -1.;
        assert_eq!(
            primal::rounded_span(generation, proof, &forged).unwrap(),
            None
        );
    }
    // H[0] and H[1] are now the same residual cell, so this witness
    // cannot bind both positive diagonals and strictly decrease their edge.
    let owner = model(
        &rows,
        2 * blocks,
        Encoding::Ordinary {
            base: 0,
            partial_right: true,
        },
    );
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    assert!(reconcile::factor_pair(
        coordinator.view().prepared,
        SeparationMethod::Simplex,
        config
    )
    .unwrap()
    .is_none());
}

#[test]
fn pure_two_intercept_simplex_certifies_all_chain_connectors_together() {
    for blocks in [16, 16_384] {
        let mut rows = Vec::new();
        for block in 0..blocks {
            for left in 0..2 {
                for right in 0..2 {
                    rows.push([
                        1.,
                        left as f64,
                        (2 * block + left) as f64,
                        (2 * block + right) as f64,
                    ]);
                }
            }
            if block + 1 < blocks {
                rows.push([0., 0., (2 * block) as f64, (2 * (block + 1)) as f64]);
            }
        }
        let owner = model(&rows, 2 * blocks, Encoding::PureAbsorbed);
        let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
        let g = coordinator.view().prepared;
        let config = SeparationConfig::default();
        let p = prepared_simplex_proposal(g, config, &mut |_| Ok(())).unwrap();
        let expected: Vec<_> = (0..rows.len()).filter(|&i| rows[i][0] == 0.).collect();
        assert_eq!(p.dropped_original_rows, expected);
        assert!(p.exact_graph_span.is_some());
        let valid = validate_numerical_proposal(
            g,
            p,
            PpmlConfig::default(),
            config,
            PreparedOlsFitConfig::default(),
        )
        .unwrap();
        assert_eq!(valid.certificate.span_residual_relative, 0.);
    }
}

#[test]
fn omitted_factor_publication_fits_its_memory_limit() {
    let rows = [
        [1., 0., 1., 1.],
        [0., 1., 0., 1.],
        [0., 2., 0., 0.],
        [1., 3., 1., 0.],
    ];
    let owner = model(
        &rows,
        2,
        Encoding::Ordinary {
            base: 0,
            partial_right: false,
        },
    );
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    let g = coordinator.view().prepared;
    let checked = |words| {
        reconcile::omitted_factor(
            g,
            SeparationMethod::Simplex,
            SeparationConfig {
                simplex_max_tableau_cells: words,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let peak = 3 * rows.len() + 1;
    assert!(checked(peak - 1).is_none());
    assert_eq!(checked(peak).unwrap().dropped_original_rows, vec![1, 2]);
    // The same search can be reached from a numerical proposal that has its
    // own larger projection-replay budget.
    let budget = primal::ProofBudget::new(
        g,
        SeparationMethod::Simplex,
        SeparationConfig {
            simplex_max_tableau_cells: peak - 1,
            ..Default::default()
        },
    );
    assert!(budget.words > peak);
    assert!(primal::omitted_factor(g, budget).unwrap().is_none());
}
