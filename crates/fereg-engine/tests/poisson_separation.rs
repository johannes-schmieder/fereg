use fereg_engine::{
    data::*,
    ppml::separation::*,
    topology::{prepare_fixed_effects, prepare_implicit_constant},
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ErrorCode, ModelSpec, SourceEntry, SourceId,
};
mod support;

fn sample(columns: &[Vec<f64>], outcome: &[f64]) -> Prepared {
    let spec = ModelSpec {
        model: CompiledTermGraph {
            sources: (0..columns.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("g{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (0..columns.len())
            .map(|i| AbsorbedTerm {
                id: AbsorbTermId::new(i as u32 + 1).unwrap(),
                intercept: true,
                categorical_sources: vec![SourceId::new(i as u32 + 1).unwrap()],
                slope_sources: vec![],
                label: format!("g{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    prepare_fixed_effects(
        &spec,
        Prepared {
            topology: None,
            n_original: outcome.len(),
            n_candidate: outcome.len(),
            original_row_by_active: (0..outcome.len()).collect(),
            outcome: outcome.to_vec(),
            regressors: columns.to_vec(),
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; outcome.len()],
        },
    )
    .unwrap()
}
fn policy() -> Policy {
    Policy {
        drop_singletons: true,
        methods: Methods::only(Method::FixedEffects),
    }
}
fn cascade_sample() -> Prepared {
    sample(
        &[
            vec![0., 0., 1., 1., 1., 2., 2., 2., 2., 2., 2.],
            vec![0., 3., 0., 1., 2., 1., 1., 2., 2., 3., 3.],
        ],
        &[0., 0., 1., 0., 0., 1., 1., 1., 1., 1., 1.],
    )
}

#[test]
fn one_generation_unites_certified_drop_singletons_and_exact_followup() {
    let mut input = cascade_sample();
    input
        .regressors
        .push((0..11).map(|i| 0.25 + i as f64).collect());
    input.weights = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: (0..11).map(|i| 1. + i as f64).collect(),
    };
    input.adjustment = PreparedAdjustment::Exposure((0..11).map(|i| 2. + i as f64).collect());
    // Availability exclusions precede the separation axis and keep their reasons.
    input.n_original = 23;
    input.n_candidate = 22;
    input.original_row_by_active = (0..11).map(|i| 2 * i + 1).collect();
    input.drop_reasons = vec![DropReason::MissingOutcome; 23];
    input.drop_reasons[0] = DropReason::ExcludedByIfIn;
    for &row in &input.original_row_by_active {
        input.drop_reasons[row] = DropReason::Retained;
    }
    let mut ledger = Ledger::new(input, policy()).unwrap();
    assert!(ledger.prune_initial_singletons().unwrap().is_none());
    let proposal = ledger.view().exact_proposal().unwrap();
    assert_eq!(proposal.rows, [1, 3]);
    assert_eq!(
        proposal.certificate.zero_only_levels,
        [ExactLevel {
            effect: Some(AbsorbTermId::new(1).unwrap()),
            representative_original_row: 1
        }]
    );
    let commit = ledger.accept_exact(&proposal).unwrap().unwrap();
    assert_eq!(
        (
            commit.generation,
            commit.primary,
            commit.singletons,
            commit.exact_followup
        ),
        (1, 2, 1, 2)
    );
    let out = ledger.view().sample();
    assert_eq!(out.retained_rows(), &[11, 13, 15, 17, 19, 21]);
    assert_eq!(out.regressors[2], [5.25, 6.25, 7.25, 8.25, 9.25, 10.25]);
    assert_eq!(
        out.weights,
        PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values: vec![6., 7., 8., 9., 10., 11.]
        }
    );
    assert_eq!(
        out.adjustment,
        PreparedAdjustment::Exposure(vec![7., 8., 9., 10., 11., 12.])
    );
    assert_eq!(out.drop_reasons[0], DropReason::ExcludedByIfIn);
    assert_eq!(out.drop_reasons[2], DropReason::MissingOutcome);
    assert_eq!(out.drop_reasons[5], DropReason::Singleton);
    for row in [1, 3, 7, 9] {
        assert_eq!(out.drop_reasons[row], DropReason::FixedEffectSeparation);
    }
    let topology = out.topology.as_ref().unwrap();
    assert_eq!(topology.partitions()[0].original_levels(), &[2]);
    assert_eq!(topology.partitions()[1].original_levels(), &[1, 2, 3]);
    assert_eq!(topology.partitions()[1].codes(), &[0, 0, 1, 1, 2, 2]);
    assert_eq!(topology.components().n_components, 1);
    topology.validate(out).unwrap();
    assert_eq!(
        ledger.accept_exact(&proposal).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    let terminal = ledger.view().exact_proposal().unwrap();
    assert_eq!(terminal.status, Status::NoSeparation);
    assert!(ledger.accept_exact(&terminal).unwrap().is_none());
    assert_eq!(ledger.trace_digest(), commit.trace_digest);
}

#[test]
fn every_malformed_row_and_certificate_rejects_the_whole_transaction() {
    let mut ledger = Ledger::new(cascade_sample(), policy()).unwrap();
    let valid = ledger.view().exact_proposal().unwrap();
    let mut mutations = vec![];
    let mut p = valid.clone();
    p.rows.push(2);
    mutations.push(p); // positive outcome
    let mut p = valid.clone();
    p.rows.push(usize::MAX);
    mutations.push(p);
    let mut p = valid.clone();
    p.rows.push(1);
    mutations.push(p); // duplicate
    let mut p = valid.clone();
    p.rows.reverse();
    mutations.push(p);
    let mut p = valid.clone();
    p.rows.pop();
    mutations.push(p); // incomplete certificate union
    let mut p = valid.clone();
    p.generation += 1;
    mutations.push(p);
    let mut p = valid.clone();
    p.iterations = 0;
    mutations.push(p);
    let mut p = valid.clone();
    p.status = Status::NoSeparation;
    mutations.push(p);
    let mut p = valid.clone();
    p.certificate.checked_intercept_effects = 0;
    mutations.push(p);
    let mut p = valid.clone();
    p.certificate.zero_only_levels[0].effect = None;
    mutations.push(p);
    let mut p = valid.clone();
    p.certificate.zero_only_levels[0].representative_original_row = 1;
    mutations.push(p);
    let mut p = valid.clone();
    p.certificate.zero_only_levels.clear();
    mutations.push(p);
    let trace = ledger.trace_digest();
    for proposal in mutations {
        assert_eq!(
            ledger.accept_exact(&proposal).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(ledger.view().sample(), &cascade_sample());
        assert_eq!(ledger.view().generation(), 0);
        assert_eq!(ledger.trace_digest(), trace);
    }
    assert!(ledger.accept_exact(&valid).unwrap().is_some());
}

#[test]
fn late_empty_sample_failure_preserves_the_accepted_owner_and_digests() {
    let original = || {
        sample(
            &[vec![0., 0., 1., 1.], vec![0., 1., 0., 1.]],
            &[0., 0., 1., 0.],
        )
    };
    let mut ledger = Ledger::new(original(), policy()).unwrap();
    let proposal = ledger.view().exact_proposal().unwrap();
    let digest = ledger.sample_digest();
    let trace = ledger.trace_digest();
    assert_eq!(
        ledger.accept_exact(&proposal).unwrap_err().code,
        ErrorCode::InsufficientObservations
    );
    assert_eq!(ledger.view().sample(), &original());
    assert_eq!(ledger.view().generation(), 0);
    assert_eq!(
        (ledger.sample_digest(), ledger.trace_digest()),
        (digest, trace)
    );
    let mut initial = Ledger::new(sample(&[vec![0., 1.]], &[1., 0.]), policy()).unwrap();
    assert_eq!(
        initial.prune_initial_singletons().unwrap_err().code,
        ErrorCode::InsufficientObservations
    );
    assert_eq!(initial.view().sample().n_active(), 2);
    assert_eq!(initial.view().generation(), 0);
}

#[test]
fn generation_digests_are_repeatable_and_include_original_row_identity() {
    let run = |shift: usize| {
        let mut input = cascade_sample();
        input.n_original += shift;
        input
            .drop_reasons
            .splice(0..0, std::iter::repeat_n(DropReason::ExcludedByIfIn, shift));
        for row in &mut input.original_row_by_active {
            *row += shift;
        }
        let mut ledger = Ledger::new(input, policy()).unwrap();
        let proposal = ledger.view().exact_proposal().unwrap();
        ledger.accept_exact(&proposal).unwrap().unwrap()
    };
    let first = run(0);
    for _ in 0..4 {
        assert_eq!(run(0), first);
    }
    let shifted = run(2);
    assert_ne!(first.sample_digest, shifted.sample_digest);
    assert_ne!(first.trace_digest, shifted.trace_digest);
    assert_eq!(
        (first.primary, first.singletons, first.exact_followup),
        (shifted.primary, shifted.singletons, shifted.exact_followup)
    );
}

#[test]
fn initial_singleton_generation_precedes_exact_and_preserves_level_identity() {
    let mut ledger = Ledger::new(
        sample(
            &[vec![0., 0., 1., 1., 2., 3., 3.]],
            &[0., 0., 1., 1., 1., 1., 1.],
        ),
        policy(),
    )
    .unwrap();
    let stale = ledger.view().exact_proposal().unwrap();
    assert_eq!(
        ledger.accept_exact(&stale).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    let initial = ledger.prune_initial_singletons().unwrap().unwrap();
    assert_eq!(
        (initial.generation, initial.singletons, initial.primary),
        (1, 1, 0)
    );
    assert_eq!(
        ledger.accept_exact(&stale).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    let proposal = ledger.view().exact_proposal().unwrap();
    let exact = ledger.accept_exact(&proposal).unwrap().unwrap();
    assert_eq!((exact.generation, exact.primary), (2, 2));
    assert_eq!(
        ledger
            .view()
            .sample()
            .topology
            .as_ref()
            .unwrap()
            .partitions()[0]
            .original_levels(),
        &[1, 3]
    );
    assert_eq!(ledger.view().sample().retained_rows(), &[2, 3, 5, 6]);
}

#[test]
fn frequency_mass_and_disabled_methods_preserve_their_policies() {
    let mut input = sample(&[vec![0., 1., 2., 2.]], &[1., 1., 0., 0.]);
    input.weights = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: vec![2., 1., 1., 1.],
    };
    let mut ledger = Ledger::new(
        input,
        Policy {
            methods: Methods::none(),
            ..policy()
        },
    )
    .unwrap();
    let commit = ledger.prune_initial_singletons().unwrap().unwrap();
    assert_eq!(commit.singletons, 1);
    assert_eq!(ledger.view().sample().retained_rows(), &[0, 2, 3]);
    let proposal = ledger.view().exact_proposal().unwrap();
    assert_eq!(proposal.rows, [2, 3]);
    assert_eq!(
        ledger.accept_exact(&proposal).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    let mut keep = Ledger::new(
        cascade_sample(),
        Policy {
            drop_singletons: false,
            ..policy()
        },
    )
    .unwrap();
    let proposal = keep.view().exact_proposal().unwrap();
    let commit = keep.accept_exact(&proposal).unwrap().unwrap();
    assert_eq!(
        (commit.primary, commit.singletons, commit.exact_followup),
        (2, 0, 0)
    );
    assert_eq!(
        keep.view().sample().retained_rows(),
        &[2, 3, 4, 5, 6, 7, 8, 9, 10]
    );
}

#[test]
fn slope_only_and_implicit_constants_have_distinct_combinatorial_support() {
    for intercept in [false, true] {
        let mut input = support::slope_sample(intercept, &[0, 0, 1, 1], &[vec![0., 1., 2., 3.]]);
        input.outcome = vec![0., 0., 1., 1.];
        let mut ledger = Ledger::new(input, policy()).unwrap();
        let proposal = ledger.view().exact_proposal().unwrap();
        if intercept {
            assert_eq!(proposal.rows, [0, 1]);
            ledger.accept_exact(&proposal).unwrap();
            assert_eq!(ledger.view().sample().regressors[1], [2., 3.]);
            assert_eq!(
                ledger.view().sample().topology.as_ref().unwrap().effects()[0].slopes[0]
                    .source
                    .get(),
                2
            );
        } else {
            assert_eq!(proposal.status, Status::NoCandidates);
            assert!(proposal.rows.is_empty());
        }
    }
    let implicit = prepare_implicit_constant(sample(&[], &[1.])).unwrap();
    let mut ledger = Ledger::new(implicit, policy()).unwrap();
    assert!(ledger.prune_initial_singletons().unwrap().is_none());
    assert_eq!(
        ledger.view().exact_proposal().unwrap().status,
        Status::NoSeparation
    );
    assert_eq!(
        Ledger::new(sample(&[], &[0., 0.]), policy())
            .unwrap_err()
            .code,
        ErrorCode::InsufficientObservations
    );
    for invalid in [-1., f64::INFINITY, f64::NAN] {
        assert_eq!(
            Ledger::new(sample(&[], &[1., invalid]), policy())
                .unwrap_err()
                .code,
            ErrorCode::InvalidData
        );
    }
}
