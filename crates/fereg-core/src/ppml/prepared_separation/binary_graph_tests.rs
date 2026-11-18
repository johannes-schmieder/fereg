use super::*;

#[test]
fn binary_graph_candidate_needs_full_model_certificate_and_explicit_admission() {
    for workers in [1, 2, 4] {
        let threads = crate::ThreadConfig::from_requested(workers, true).unwrap();
        crate::parallel::ThreadTeam::new(threads)
            .install(|| check_binary_candidate(threads))
            .unwrap();
    }
}

fn check_binary_candidate(threads: crate::ThreadConfig) {
    let (mut request, clusters, mut loaded, mut uses) = fixture();
    let mut second = request.absorbed_terms[0].clone();
    second.id = AbsorbTermId::new(2).unwrap();
    second.categorical_sources = vec![SourceId::new(3).unwrap()];
    request.absorbed_terms.push(second);
    request.keep_singletons = true;
    loaded.candidate = vec![true; 8].into();
    loaded.outcome = column(&[1., 1., 1., 1., 1., 0., 0., 0.]);
    loaded.numeric_sources[0].1 = column(&[0., 0., 1., 1., 1., 0., 0., 0.]);
    loaded.numeric_sources[1].1 = column(&[1., 1., 2., 2., 3., 3., 3., 1.]);
    loaded.numeric_sources[2].1 = column(&[1., 2., 1., 2., 1., 1., 2., 1.]);
    uses[2].fixed_effect = true;
    let owner = PreparedPpmlModelOwner::new(
        request,
        clusters,
        loaded.clone(),
        prepare_numeric(&loaded, &uses),
    )
    .unwrap();
    let coordinator = PreparedSeparationCoordinator::new(owner).unwrap();
    let generation = coordinator.view().prepared;
    let mut ppml = PpmlConfig::default();
    ppml.absorb.lsmr.threads = threads;
    ppml.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: 1_000_000,
    });
    let linear = PreparedOlsFitConfig {
        execution: crate::ols::PreparedOlsExecutionConfig {
            threads,
            ..Default::default()
        },
        absorb: ppml.absorb,
        fixed_effect_collinearity_tolerance: ppml.fixed_effect_collinearity_tolerance,
        ..PreparedOlsFitConfig::default()
    };
    let config = rectifier_only();
    for admitted in [false, true] {
        let proposal = prepared_bound_iterated_rectifier(
            generation,
            ppml,
            config,
            linear,
            admitted,
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(proposal.dropped_original_rows, &[5, 6]);
        assert_eq!(
            proposal
                .exact_graph_span
                .as_ref()
                .is_some_and(|span| span.has_binary_column()),
            admitted
        );
        assert_eq!(proposal.iterations == 0, admitted);
        validate_numerical_proposal(generation, proposal, ppml, config, linear).unwrap();
    }
    // One byte below the admitted graph envelope must retain the unit
    // route, even when its separate projector workspace remains ample.
    let design = generation
        .fit_view()
        .fit_borrowed_intercept_map_design()
        .unwrap()
        .unwrap();
    let graph_bytes = graph::workspace_bytes(&design).unwrap().unwrap();
    let mut limited = linear;
    limited.absorb.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
        execution_bytes: 0,
        usable_bytes: graph_bytes - 1,
    });
    let proposal =
        prepared_bound_iterated_rectifier(generation, ppml, config, limited, true, &mut |_| Ok(()))
            .unwrap();
    assert!(proposal.iterations > 0);
    assert_eq!(proposal.dropped_original_rows, &[5, 6]);
    // Independent raw coefficient replay rejects a forged strict row.
    let mut forged =
        prepared_bound_iterated_rectifier(generation, ppml, config, linear, true, &mut |_| Ok(()))
            .unwrap();
    forged.direction.as_mut().unwrap()[7] = -0.5;
    forged.dropped_original_rows.push(7);
    forged.certificate = Some(
        detector_direction_certificate(
            generation.outcome(),
            forged.direction.as_ref().unwrap(),
            &[5, 6, 7],
            config.certificate_tolerance,
        )
        .unwrap(),
    );
    validate_prepared_numerical_metadata(generation, &forged, config).unwrap();
    // Row7 duplicates the complete design of positive row0. The proposed
    // negative value is sign-valid but cannot belong to the model span.
    let error = validate_numerical_proposal(generation, forged, ppml, config, linear).unwrap_err();
    assert!(
        error
            .message
            .contains("coefficient replay disagrees with direction"),
        "{error}"
    );
}
