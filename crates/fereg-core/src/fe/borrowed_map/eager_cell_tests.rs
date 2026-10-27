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
            assert!(owner.reuse.cells().unwrap().attempted);
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
    for deferred in [false, true] {
        let fixture = PortfolioRectangle::new();
        let mut owner = ReusableCategoricalMapAbsorber::new(fixture.design(), 2, 1).unwrap();
        owner.retain_coefficients = true;
        owner.request_auto_routing(RoutingPreset::portfolio_v1(), None, 0);
        if deferred {
            owner
                .request_deferred_cells(BorrowedMapWeights::Unit)
                .unwrap();
        } else {
            request_cells(&mut owner, BorrowedMapWeights::Unit, None);
        }
        let zero = DenseMatrix::zeros(fixture.values.nrows(), 1).unwrap();
        assert_eq!(
            owner
                .absorb_ols(BorrowedMapWeights::Unit, zero.into(), config(), false)
                .unwrap()
                .report()
                .iterations,
            0
        );
        assert!(!owner.reuse.cells().unwrap().attempted);
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
        assert!(!owner.reuse.cells().unwrap().attempted);
        assert!(owner.two_fe_pcg.as_ref().unwrap().kernel.is_none());
    }
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
        new.reuse.cells().unwrap().refusal,
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
    for mode in [
        crate::ols::PreparedOlsCells::Off,
        crate::ols::PreparedOlsCells::Eager,
        crate::ols::PreparedOlsCells::Deferred,
    ] {
        let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
        owner.retain_coefficients = true;
        owner.request_auto_routing(RoutingPreset::gates_two(), None, 0);
        match mode {
            crate::ols::PreparedOlsCells::Off => {}
            crate::ols::PreparedOlsCells::Eager => owner
                .request_eager_cells(BorrowedMapWeights::Provided(&witness.weights))
                .unwrap(),
            crate::ols::PreparedOlsCells::Deferred => owner
                .request_deferred_cells(BorrowedMapWeights::Provided(&witness.weights))
                .unwrap(),
        }
        let mut state = crate::fe::routing_gates::GateState::new(
            crate::fe::routing_gates::GateRule::Structural16,
        );
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
        assert_eq!(
            builds,
            usize::from(mode != crate::ols::PreparedOlsCells::Off)
        );
        assert_eq!(applies > 0, mode != crate::ols::PreparedOlsCells::Off);
        let graph = owner.two_fe_pcg.as_ref().unwrap();
        let cmg = graph
            .cmg_schur
            .as_ref()
            .expect("real GATES checkpoint must install");
        hierarchies.push(cmg.trace_hierarchy());
        let retained = graph.kernel.as_ref().map_or(0, |k| k.retained_bytes());
        assert!(
            owner.cmg_schur_peak().unwrap().unwrap().bytes().unwrap() as u128
                >= cmg.retained_bytes().unwrap() + retained
        );
        if mode != crate::ols::PreparedOlsCells::Deferred {
            assert!(
                owner.auto_route.as_ref().unwrap().peak.bytes().unwrap() as u128
                    >= cmg.construction_peak().bytes().unwrap() as u128 + retained
            );
        } else {
            // Subsequent Low-style slices reuse the installed bank, and reject
            // another weight owner even when its numerical values are equal.
            let address = graph.kernel.as_ref().unwrap() as *const _;
            let (again, builds, _) = crate::fe::routing_kernel::recorded_activity(|| {
                owner
                    .absorb_ols(
                        BorrowedMapWeights::Provided(&witness.weights),
                        witness.columns(&[1]).into(),
                        config(),
                        false,
                    )
                    .unwrap()
            });
            witness.check(&again, &[1]);
            assert_eq!(builds, 0);
            assert_eq!(
                address,
                owner.two_fe_pcg.as_ref().unwrap().kernel.as_ref().unwrap() as *const _
            );
            let other = witness.weights.clone();
            assert_eq!(
                owner
                    .absorb_ols(
                        BorrowedMapWeights::Provided(&other),
                        witness.columns(&[1]).into(),
                        config(),
                        false
                    )
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidArgument
            );
        }
    }
    assert_eq!(hierarchies[0], hierarchies[1]);
    assert_eq!(hierarchies[0], hierarchies[2]);
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

fn deferred_solve(
    owner: &mut ReusableCategoricalMapAbsorber<'_>,
    witness: &PairedGraph,
) -> Result<CertifiedBorrowedMap> {
    let mut state =
        crate::fe::routing_gates::GateState::new(crate::fe::routing_gates::GateRule::Structural16);
    state.begin(
        crate::fe::routing_remaining::Demand::Packed { regressors: 2 },
        3,
        3,
        false,
    );
    owner.absorb_ols_with_remaining(
        BorrowedMapWeights::Provided(&witness.weights),
        witness.input.clone().into(),
        config(),
        false,
        Some(ObserverRef::Gates(&mut state)),
    )
}

#[test]
fn deferred_cells_require_successful_cmg_commitment_and_preserve_refusal_bits() {
    if isolated_routing_witness("FEREG_DEFERRED_REFUSAL_CHILD",
        "fe::borrowed_map::routing_tests::eager_cell_tests::deferred_cells_require_successful_cmg_commitment_and_preserve_refusal_bits") { return; }
    let witness = PairedGraph::new();
    // Neither an ineligible grant nor a refused CMG build can construct cells.
    for budget in [
        None,
        Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 0,
        }),
    ] {
        let mut fits = Vec::new();
        for deferred in [false, true] {
            let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
            owner.retain_coefficients = true;
            owner.request_auto_routing(RoutingPreset::gates_two(), budget, 0);
            if deferred {
                owner
                    .request_deferred_cells(BorrowedMapWeights::Provided(&witness.weights))
                    .unwrap();
            }
            let (fit, builds, applies) = crate::fe::routing_kernel::recorded_activity(|| {
                if budget.is_some() {
                    deferred_solve(&mut owner, &witness).unwrap()
                } else {
                    owner
                        .absorb_ols(
                            BorrowedMapWeights::Provided(&witness.weights),
                            witness.input.clone().into(),
                            config(),
                            false,
                        )
                        .unwrap()
                }
            });
            witness.check(&fit, &[0, 1, 2]);
            assert_eq!((builds, applies), (0, 0));
            assert!(owner.two_fe_pcg.as_ref().unwrap().cmg_schur.is_none());
            assert_eq!(
                owner.auto_route.as_ref().unwrap().setup.attempted,
                budget.is_some()
            );
            if deferred {
                assert!(!owner.reuse.cells().unwrap().attempted);
            }
            fits.push(fit);
        }
        assert_same_bits(
            fits[0].residualized().as_slice(),
            fits[1].residualized().as_slice(),
        );
        assert_eq!(fits[0].report().iterations, fits[1].report().iterations);
    }
    // Each constructor fits alone, but cells plus installed CMG do not.
    // Refusal must retain that preconditioner and the observation bits.
    let mut reference = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    reference.retain_coefficients = true;
    reference.request_auto_routing(RoutingPreset::gates_two(), None, 0);
    let expected = deferred_solve(&mut reference, &witness).unwrap();
    let cmg_peak = reference.auto_route.as_ref().unwrap().peak.bytes().unwrap() as u128;
    let cells = crate::fe::routing_kernel::build_occurrence(
        &witness.design(),
        reference.two_fe_pcg.as_ref().unwrap().left_effect,
        BorrowedMapWeights::Provided(&witness.weights),
        validate_weights(
            BorrowedMapWeights::Provided(&witness.weights),
            witness.input.nrows(),
        )
        .unwrap()
        .raw_scale,
        crate::fe::routing_kernel::KernelLimits {
            retained_live_bytes: 0,
            memory: None,
        },
    )
    .unwrap();
    assert!(cells.kernel.is_some());
    let cell_peak = cells.peak.bytes().unwrap() as u128;
    let peak = cmg_peak.max(cell_peak);
    assert!(
        cell_peak + reference.cmg_schur_bytes().unwrap() > peak,
        "fixture must discriminate missing coexistence accounting"
    );
    drop(cells);
    let hierarchy = reference
        .two_fe_pcg
        .as_ref()
        .unwrap()
        .cmg_schur
        .as_ref()
        .unwrap()
        .trace_hierarchy();
    let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
    owner.retain_coefficients = true;
    owner.request_auto_routing(
        RoutingPreset::gates_two(),
        Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: peak,
        }),
        0,
    );
    owner
        .request_deferred_cells(BorrowedMapWeights::Provided(&witness.weights))
        .unwrap();
    let actual = deferred_solve(&mut owner, &witness).unwrap();
    assert_same_bits(
        actual.residualized().as_slice(),
        expected.residualized().as_slice(),
    );
    assert_eq!(actual.report().iterations, expected.report().iterations);
    let graph = owner.two_fe_pcg.as_ref().unwrap();
    assert_eq!(
        graph.cmg_schur.as_ref().unwrap().trace_hierarchy(),
        hierarchy
    );
    assert!(graph.kernel.is_none() && !graph.use_cell_operator);
    assert!(owner.reuse.cells().unwrap().attempted);
    assert_eq!(
        owner.reuse.cells().unwrap().refusal,
        Some(crate::fe::routing_kernel::KernelRefusal::MemoryBudget)
    );
    assert!(owner.cmg_schur_peak().unwrap().unwrap().bytes().unwrap() as u128 <= peak);
}

#[test]
fn deferred_cells_cancel_or_fail_after_install_without_publishing_kernel_and_rebuild() {
    if isolated_routing_witness("FEREG_DEFERRED_FATAL_CHILD",
        "fe::borrowed_map::routing_tests::eager_cell_tests::deferred_cells_cancel_or_fail_after_install_without_publishing_kernel_and_rebuild") { return; }
    let witness = PairedGraph::new();
    for cancel in [true, false] {
        let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
        owner.retain_coefficients = true;
        owner.request_auto_routing(RoutingPreset::gates_two(), None, 0);
        owner
            .request_deferred_cells(BorrowedMapWeights::Provided(&witness.weights))
            .unwrap();
        TWO_FE_ROUTING_BUILD_AT.set(0);
        let (fit, work) = crate::fe::routing_kernel::construction_fault(cancel, || {
            deferred_solve(&mut owner, &witness)
        });
        assert_eq!(
            fit.unwrap_err().code,
            if cancel {
                ErrorCode::Interrupted
            } else {
                ErrorCode::Allocation
            }
        );
        assert_eq!(work, if cancel { 32 } else { 0 });
        assert_eq!(TWO_FE_ROUTING_BUILD_AT.get(), 16);
        let graph = owner.two_fe_pcg.as_ref().unwrap();
        assert!(
            graph.cmg_schur.is_some(),
            "fatal cell work must be after CMG installation"
        );
        assert!(graph.kernel.is_none() && !graph.use_cell_operator);
        drop(owner);
        let mut fresh = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
        fresh.retain_coefficients = true;
        fresh.request_auto_routing(RoutingPreset::gates_two(), None, 0);
        fresh
            .request_deferred_cells(BorrowedMapWeights::Provided(&witness.weights))
            .unwrap();
        witness.check(&deferred_solve(&mut fresh, &witness).unwrap(), &[0, 1, 2]);
        assert!(fresh.two_fe_pcg.as_ref().unwrap().kernel.is_some());
    }
}

#[test]
fn deferred_cells_follow_correction_commit_without_reopening_gates() {
    if isolated_routing_witness("FEREG_DEFERRED_CORRECTION_CHILD",
        "fe::borrowed_map::routing_tests::eager_cell_tests::deferred_cells_follow_correction_commit_without_reopening_gates") { return; }
    let witness = PairedGraph::new();
    for preset in [
        RoutingPreset::new(16, 1).unwrap(),
        RoutingPreset::gates_two(),
    ] {
        let mut owner = ReusableCategoricalMapAbsorber::new(witness.design(), 3, 1).unwrap();
        owner.retain_coefficients = true;
        owner.request_auto_routing(preset, None, 0);
        owner
            .request_deferred_cells(BorrowedMapWeights::Provided(&witness.weights))
            .unwrap();
        // Force the original-design certificate before the ordinary h16
        // checkpoint. Only the existing correction checkpoint can install at 16.
        TWO_FE_CERTIFICATE_AT.set(16);
        TWO_FE_CERTIFICATE_REJECTED_AT.set(0);
        TWO_FE_ROUTING_BUILD_AT.set(0);
        TWO_FE_CORRECTIONS.set(0);
        let (fit, builds, applies) = crate::fe::routing_kernel::recorded_activity(|| {
            deferred_solve(&mut owner, &witness).unwrap()
        });
        witness.check(&fit, &[0, 1, 2]);
        assert_eq!(TWO_FE_CERTIFICATE_AT.get(), 0);
        assert_eq!(TWO_FE_CERTIFICATE_REJECTED_AT.get(), 16);
        assert_eq!(TWO_FE_CORRECTIONS.get(), 1);
        let installed = !preset.requires_economic_grant();
        assert_eq!(
            TWO_FE_ROUTING_BUILD_AT.get(),
            if installed { 16 } else { 0 }
        );
        assert_eq!(builds, usize::from(installed));
        assert_eq!(applies > 0, installed);
        let graph = owner.two_fe_pcg.as_ref().unwrap();
        assert_eq!(graph.cmg_schur.is_some(), installed);
        assert_eq!(graph.kernel.is_some(), installed);
        assert_eq!(owner.reuse.cells().unwrap().attempted, installed);
    }
}
