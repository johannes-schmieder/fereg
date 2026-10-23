use super::*;

fn request_cells<'a>(
    owner: &mut ReusableCategoricalMapAbsorber<'a>,
    weights: BorrowedMapWeights<'a>,
    budget: Option<FeSolverMemoryBudget>,
) {
    if owner.auto_route.is_none() {
        owner.request_auto_routing(
            RoutingPreset::prefix(crate::fe::routing::CategoricalPrefix::None),
            budget,
            0,
        );
    } else {
        owner.auto_route.as_mut().unwrap().request.budget = budget;
    }
    owner.request_eager_cells(weights).unwrap();
}

#[test]
fn eager_cells_reuse_weighted_disconnected_low_chunks_and_reject_other_owner() {
    let witness = PairedGraph::new();
    for threads in [1, 4] {
        let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, threads).unwrap();
        owner.retain_coefficients = true;
        request_cells(
            &mut owner,
            BorrowedMapWeights::Provided(&witness.weights),
            None,
        );
        let mut pointer = None;
        for positions in [&[0, 1][..], &[1][..], &[0, 1][..]] {
            let fit = owner
                .absorb_ols(
                    BorrowedMapWeights::Provided(&witness.weights),
                    witness.columns(positions).into(),
                    config(),
                    false,
                )
                .unwrap();
            witness.check(&fit, positions);
            let graph = owner.two_fe_pcg.as_ref().unwrap();
            assert!(graph.use_cell_operator);
            let kernel = graph.kernel.as_ref().unwrap();
            let address = kernel as *const _;
            if let Some(prior) = pointer {
                assert_eq!(prior, address);
            }
            pointer = Some(address);
            assert!(owner.reuse.eager().unwrap().attempted);
            assert!(
                owner.cmg_schur_peak().unwrap().unwrap().bytes().unwrap() as u128
                    >= kernel.retained_bytes()
            );
        }
        let other_weights = witness.weights.clone();
        let error = owner
            .absorb_ols(
                BorrowedMapWeights::Provided(&other_weights),
                witness.columns(&[0]).into(),
                config(),
                false,
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }
}

#[test]
fn eager_cells_initial_ready_and_direct_selection_do_not_construct() {
    let fixture = PortfolioRectangle::new();
    let mut owner = ReusableCategoricalMapAbsorber::new(fixture.design(), 2, 1).unwrap();
    owner.retain_coefficients = true;
    owner.request_auto_routing(RoutingPreset::portfolio_v1(), None, 0);
    request_cells(&mut owner, BorrowedMapWeights::Unit, None);
    let zero = DenseMatrix::zeros(fixture.values.nrows(), 1).unwrap();
    assert_eq!(
        owner
            .absorb_ols(BorrowedMapWeights::Unit, zero.into(), config(), false)
            .unwrap()
            .report()
            .iterations,
        0
    );
    assert!(!owner.reuse.eager().unwrap().attempted);
    let direct = owner
        .absorb_ols(
            BorrowedMapWeights::Unit,
            fixture.first().into(),
            config(),
            false,
        )
        .unwrap();
    fixture.check(&direct);
    assert_eq!(direct.report().iterations, 0);
    assert!(!owner.reuse.eager().unwrap().attempted);
    assert!(owner.two_fe_pcg.as_ref().unwrap().kernel.is_none());
}

#[test]
fn eager_cells_refusal_preserves_observation_bits_and_does_not_consume_cmg_attempt() {
    let witness = PairedGraph::new();
    let mut old = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    old.request_auto_routing(
        RoutingPreset::prefix(crate::fe::routing::CategoricalPrefix::None),
        None,
        0,
    );
    let expected = old
        .absorb_ols(
            BorrowedMapWeights::Provided(&witness.weights),
            witness.input.clone().into(),
            config(),
            false,
        )
        .unwrap();
    let mut new = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    request_cells(
        &mut new,
        BorrowedMapWeights::Provided(&witness.weights),
        Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 0,
        }),
    );
    let actual = new
        .absorb_ols(
            BorrowedMapWeights::Provided(&witness.weights),
            witness.input.clone().into(),
            config(),
            false,
        )
        .unwrap();
    assert_same_bits(
        actual.residualized().as_slice(),
        expected.residualized().as_slice(),
    );
    assert_eq!(actual.report().iterations, expected.report().iterations);
    assert_eq!(
        new.reuse.eager().unwrap().refusal,
        Some(super::super::super::routing_kernel::KernelRefusal::MemoryBudget)
    );
    new.request_auto_routing(RoutingPreset::new(16, 1).unwrap(), None, 0);
    assert!(!new.auto_route.as_ref().unwrap().setup.attempted);
}

#[test]
fn eager_retained_cells_are_admitted_and_reported_through_cmg_checkpoint() {
    let fixture = PortfolioRectangle::new();
    let design = fixture.design();
    let reference = CmgSchurPreconditioner::try_build_bounded(
        &design,
        BorrowedMapWeights::Unit,
        0,
        5,
        super::super::super::cmg_schur::Sampling {
            samples: 1,
            seed: 0,
        },
        2,
        None,
        false,
    )
    .unwrap();
    assert!(reference.preconditioner.is_some());
    let cmg_peak = reference.peak.bytes().unwrap() as u128;
    for retained in [0, cmg_peak + 1] {
        let mut owner = ReusableCategoricalMapAbsorber::new(fixture.design(), 2, 1).unwrap();
        owner.request_auto_routing(
            RoutingPreset::new(16, 1).unwrap(),
            Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: cmg_peak,
            }),
            0,
        );
        let route = owner.auto_route.as_mut().unwrap();
        let built = route
            .checkpoint(
                &design,
                validate_weights(BorrowedMapWeights::Unit, design.nobs()).unwrap(),
                0,
                2,
                16,
                200,
                false,
                retained,
                None,
                &mut 0.,
                &mut Instant::now(),
            )
            .unwrap();
        assert_eq!(built.is_some(), retained == 0);
        assert!(route.setup.attempted);
        assert!(route.peak.bytes().unwrap() as u128 >= retained);
    }
}

#[test]
fn eager_cells_gates_h16_installs_the_original_graph_hierarchy() {
    if isolated_routing_witness("FEREG_EAGER_GATES_HIERARCHY_CHILD",
        "fe::borrowed_map::routing_tests::eager_cell_tests::eager_cells_gates_h16_installs_the_original_graph_hierarchy") { return; }
    let witness = PairedGraph::new();
    let mut hierarchies = Vec::new();
    for eager in [false, true] {
        let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
        owner.retain_coefficients = true;
        owner.request_auto_routing(RoutingPreset::gates_two(), None, 0);
        if eager {
            owner
                .request_eager_cells(BorrowedMapWeights::Provided(&witness.weights))
                .unwrap();
        }
        let mut state = crate::fe::routing_gates::GateState::new();
        state.begin(
            crate::fe::routing_remaining::Demand::Packed { regressors: 2 },
            3,
            3,
            false,
        );
        TWO_FE_ROUTING_BUILD_AT.set(0);
        let (fit, builds, applies) = crate::fe::routing_kernel::recorded_activity(|| {
            owner
                .absorb_ols_with_remaining(
                    BorrowedMapWeights::Provided(&witness.weights),
                    witness.input.clone().into(),
                    config(),
                    false,
                    Some(ObserverRef::Gates(&mut state)),
                )
                .unwrap()
        });
        witness.check(&fit, &[0, 1, 2]);
        assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 16);
        assert_eq!(builds, usize::from(eager));
        assert_eq!(applies > 0, eager);
        let graph = owner.two_fe_pcg.as_ref().unwrap();
        let cmg = graph
            .cmg_schur
            .as_ref()
            .expect("real GATES checkpoint must install");
        hierarchies.push(cmg.trace_hierarchy());
        let retained = graph.kernel.as_ref().map_or(0, |k| k.retained_bytes());
        assert!(
            owner.auto_route.as_ref().unwrap().peak.bytes().unwrap() as u128
                >= cmg.construction_peak().bytes().unwrap() as u128 + retained
        );
    }
    assert_eq!(hierarchies[0], hierarchies[1]);
}

#[test]
fn eager_cells_and_ppml_reuse_owners_cannot_replace_each_other() {
    let witness = PairedGraph::new();
    let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    owner.request_auto_routing(RoutingPreset::gates_two(), None, 0);
    owner
        .request_eager_cells(BorrowedMapWeights::Provided(&witness.weights))
        .unwrap();
    assert!(owner
        .request_eager_cells(BorrowedMapWeights::Provided(&witness.weights))
        .is_err());
    let generation = crate::fe::MapGeometryGeneration {
        sample: 0,
        topology: 0,
        weights: 0,
    };
    assert!(owner.enable_ppml_topology(generation, 1_000_000).is_err());
    let mut ppml = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    ppml.enable_ppml_topology(generation, 1_000_000).unwrap();
    ppml.request_auto_routing(RoutingPreset::gates_two(), None, 0);
    assert!(ppml
        .request_eager_cells(BorrowedMapWeights::Provided(&witness.weights))
        .is_err());
}
