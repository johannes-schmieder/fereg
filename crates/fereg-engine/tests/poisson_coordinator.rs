use fereg_engine::{
    data::*,
    ppml::{irls, separation::*},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    wls::Role,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ErrorCode, ModelSpec, SourceEntry, SourceId,
};
mod support;
fn columns(recipes: Vec<ColumnRecipe>, roles: Vec<Role>) -> CompiledColumns {
    CompiledColumns {
        slots: (0..recipes.len())
            .map(|i| ColumnSlot {
                name: format!("x{i}"),
                syntax: SyntaxStatus::Active,
                generated: Some(i),
            })
            .collect(),
        constant_slot: recipes.iter().position(|r| *r == ColumnRecipe::Intercept),
        recipes,
        roles,
        implicit_constant: false,
        structural_intercept: true,
    }
}
fn categorical(codes: &[Vec<u32>], y: &[f64]) -> Prepared {
    let spec = ModelSpec {
        model: CompiledTermGraph {
            sources: (0..codes.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("g{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (0..codes.len())
            .map(|i| AbsorbedTerm {
                id: AbsorbTermId::new(i as u32 + 1).unwrap(),
                categorical_sources: vec![SourceId::new(i as u32 + 1).unwrap()],
                intercept: true,
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
            n_original: y.len(),
            n_candidate: y.len(),
            original_row_by_active: (0..y.len()).collect(),
            outcome: y.to_vec(),
            regressors: codes
                .iter()
                .map(|c| c.iter().map(|&v| f64::from(v)).collect())
                .collect(),
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; y.len()],
        },
    )
    .unwrap()
}
fn scalar(y: &[f64]) -> Prepared {
    categorical(&[vec![0; y.len()]], y)
}
fn options(solver: Solver) -> Options {
    Options {
        solver,
        ..Options::for_estimator(Estimator::Ppml)
    }
}
fn policy(methods: Methods) -> Policy {
    Policy {
        drop_singletons: false,
        methods,
    }
}
fn const_columns() -> CompiledColumns {
    columns(vec![ColumnRecipe::Intercept], vec![Role::AbsorbedIntercept])
}
fn ray(y: &[f64]) -> (Prepared, CompiledColumns) {
    let mut s = scalar(y);
    s.regressors
        .push(y.iter().map(|&v| if v == 0. { -1. } else { 0. }).collect());
    (
        s,
        columns(
            vec![
                ColumnRecipe::Continuous { source: 1 },
                ColumnRecipe::Intercept,
            ],
            vec![Role::Ordinary, Role::AbsorbedIntercept],
        ),
    )
}
#[test]
fn canonical_methods_and_atomic_cascades_rebuild_the_final_columns_and_rank() {
    let mut s = categorical(
        &[
            vec![0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 2],
            vec![0, 3, 0, 1, 2, 1, 1, 2, 2, 3, 3],
        ],
        &[0., 0., 1., 0., 0., 0., 1., 1., 1., 1., 1.],
    );
    let mut x = vec![0.; 11];
    x[5] = -1.;
    s.regressors.push(x);
    s.weights = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: (1..=11).map(f64::from).collect(),
    };
    s.adjustment = PreparedAdjustment::Exposure((1..=11).map(f64::from).collect());
    s.n_original = 23;
    s.n_candidate = 22;
    s.original_row_by_active = (0..11).map(|i| 2 * i + 1).collect();
    s.drop_reasons = vec![DropReason::MissingOutcome; 23];
    s.drop_reasons[0] = DropReason::ExcludedByIfIn;
    for &r in &s.original_row_by_active {
        s.drop_reasons[r] = DropReason::Retained;
    }
    let c = columns(
        vec![
            ColumnRecipe::Continuous { source: 2 },
            ColumnRecipe::Intercept,
        ],
        vec![Role::Ordinary, Role::AbsorbedIntercept],
    );
    let mut reference = None;
    for (workers, methods) in [
        (1, "mu ir simplex fe"),
        (2, "fe simplex ir mu"),
        (4, "ir mu fe simplex"),
        (8, "simplex fe mu ir"),
    ] {
        let team = ThreadTeam::new(workers).unwrap();
        // The test's fixture is reconstructed from the same original sources,
        // rather than from any prior detector's compacted or fitted state.
        let input = copy_sample(&s);
        let result = run(
            input,
            &c,
            options(Solver::Auto),
            Policy {
                drop_singletons: true,
                methods: Methods::parse(methods).unwrap(),
            },
            RunControls::default(),
            &team,
        )
        .unwrap();
        assert_eq!(result.sample().retained_rows(), [15, 17, 19, 21]);
        assert_eq!(result.sample().regressors[2], [0.; 4]);
        assert_eq!(result.raw().column(0).unwrap(), [0.; 4]);
        assert_eq!(
            result.sample().adjustment,
            PreparedAdjustment::Exposure(vec![8., 9., 10., 11.])
        );
        let report = result.report();
        assert_eq!(
            (
                report.generation,
                report.rounds,
                report.singletons,
                report.exact_followup
            ),
            (2, 3, 2, 2)
        );
        assert_eq!(report.methods[0].primary, 2);
        assert_eq!(report.methods[1].primary, 1);
        assert_eq!(report.methods[0].calls, 3);
        assert_eq!(report.methods[1].calls, 2);
        let detectors: Vec<_> = report
            .trace
            .iter()
            .flatten()
            .filter(|e| e.kind == EventKind::Detector)
            .map(|e| e.method.unwrap())
            .collect();
        assert_eq!(
            detectors,
            [
                Method::FixedEffects,
                Method::FixedEffects,
                Method::Simplex,
                Method::FixedEffects,
                Method::Simplex,
                Method::IteratedRectifier,
                Method::Mu
            ]
        );
        assert_eq!(
            result.sample().drop_reasons[11],
            DropReason::SimplexSeparation
        );
        for r in [5, 13] {
            assert_eq!(result.sample().drop_reasons[r], DropReason::Singleton);
        }
        for r in [1, 3, 7, 9] {
            assert_eq!(
                result.sample().drop_reasons[r],
                DropReason::FixedEffectSeparation
            );
        }
        let fit = result.fit(&team).unwrap();
        assert_eq!(fit.point.finite, [0., 0.]);
        assert!(fit.linear.finite.kept.is_empty());
        assert!(fit.diagnostics.strict_checks >= 2);
        let cold = irls::fit_final_sample(
            &irls::Problem {
                sample: result.sample(),
                raw: result.raw(),
                roles: result.roles(),
            },
            options(Solver::Auto),
            &team,
        )
        .unwrap();
        assert_eq!(fit, cold);
        if let Some(prior) = &reference {
            assert_eq!(prior, &(result, fit));
        } else {
            reference = Some((result, fit));
        }
    }
}
fn copy_sample(s: &Prepared) -> Prepared {
    Prepared {
        topology: s.topology.clone(),
        n_original: s.n_original,
        n_candidate: s.n_candidate,
        original_row_by_active: s.original_row_by_active.clone(),
        outcome: s.outcome.clone(),
        regressors: s.regressors.clone(),
        weights: match &s.weights {
            PreparedWeights::None => PreparedWeights::None,
            PreparedWeights::Provided { kind, values } => PreparedWeights::Provided {
                kind: *kind,
                values: values.clone(),
            },
        },
        adjustment: match &s.adjustment {
            PreparedAdjustment::None => PreparedAdjustment::None,
            PreparedAdjustment::Offset(v) => PreparedAdjustment::Offset(v.clone()),
            PreparedAdjustment::Exposure(v) => PreparedAdjustment::Exposure(v.clone()),
        },
        drop_reasons: s.drop_reasons.clone(),
    }
}
#[test]
fn simplex_capacity_needs_same_generation_ir_and_a_drop_restarts_its_check() {
    let team = ThreadTeam::new(1).unwrap();
    let controls = RunControls {
        simplex: SimplexLimits {
            max_cells: 1,
            ..SimplexLimits::default()
        },
        ..RunControls::default()
    };
    let c = const_columns();
    let complete = run(
        scalar(&[0., 1., 0., 1.]),
        &c,
        options(Solver::Auto),
        policy(Methods::parse("simplex ir").unwrap()),
        controls,
        &team,
    )
    .unwrap();
    let r = complete.report();
    assert_eq!(r.generation, 0);
    assert_eq!(r.capacity_resolutions, 1);
    assert_eq!(r.methods[1].final_status, Some(Status::SkippedCapacity));
    assert_eq!(r.methods[2].final_status, Some(Status::NoSeparation));
    assert_eq!((r.methods[2].calls, r.methods[2].replays), (1, 1));
    assert_eq!(r.work.projection().calls(), 4);
    assert!(complete
        .fit(&team)
        .unwrap()
        .evaluation
        .mean
        .iter()
        .all(|mu| (mu - 0.5).abs() <= 1e-12));
    for methods in ["simplex", "simplex mu"] {
        assert_eq!(
            run(
                scalar(&[0., 1.]),
                &c,
                options(Solver::Auto),
                policy(Methods::parse(methods).unwrap()),
                controls,
                &team
            )
            .unwrap_err()
            .code,
            ErrorCode::Allocation
        );
    }
    let (s, c) = ray(&[0., 1., 1., 1.]);
    let done = run(
        s,
        &c,
        options(Solver::Auto),
        policy(Methods::parse("simplex ir").unwrap()),
        controls,
        &team,
    )
    .unwrap();
    assert_eq!(done.sample().retained_rows(), [1, 2, 3]);
    assert_eq!(
        done.sample().drop_reasons[0],
        DropReason::IteratedRectifierSeparation
    );
    let r = done.report();
    assert_eq!(r.capacity_resolutions, 0);
    assert_eq!((r.methods[1].calls, r.methods[1].replays), (2, 1));
    assert_eq!((r.methods[2].calls, r.methods[2].replays), (2, 1));
    assert_eq!(r.methods[1].last_generation, Some(1));
    assert_eq!(r.methods[1].final_status, Some(Status::NoCandidates));
}
#[test]
fn mu_discards_its_fitted_session_before_the_fresh_final_sample_fit() {
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let mut s = support::prepared(&[7, 5], false, 20, 0);
        let n = s.n_active();
        s.outcome[0] = 0.;
        let source = s.regressors.len();
        let mut x = vec![0.; n];
        x[0] = -1.;
        s.regressors.push(x);
        let c = columns(
            vec![ColumnRecipe::Continuous { source }, ColumnRecipe::Intercept],
            vec![Role::Ordinary, Role::AbsorbedIntercept],
        );
        let team = ThreadTeam::new(1).unwrap();
        let out = run(
            s,
            &c,
            options(solver),
            policy(Methods::only(Method::Mu)),
            RunControls::default(),
            &team,
        )
        .unwrap();
        assert_eq!(out.sample().drop_reasons[0], DropReason::MuSeparation);
        assert_eq!(
            (
                out.report().methods[3].calls,
                out.report().methods[3].replays
            ),
            (2, 1)
        );
        let root = out.fit(&team).unwrap();
        assert_eq!(root.point.finite, [0., 0.]);
        assert_eq!(root.evaluation.mean, vec![1.; n - 1]);
        assert!(root.diagnostics.accepted_iterations < out.report().methods[3].iterations);
        assert!(
            out.report().work.projection().counts().right_hand_sides
                > root.diagnostics.work.projection().counts().right_hand_sides
        );
    }
}
#[test]
fn none_disables_detectors_but_keeps_multiplicity_and_final_fit_guards() {
    let team = ThreadTeam::new(1).unwrap();
    for frequency in [false, true] {
        let mut s = categorical(&[vec![0, 1, 1]], &[0., 1., 1.]);
        if frequency {
            s.weights = PreparedWeights::Provided {
                kind: WeightKind::Frequency,
                values: vec![2.; 3],
            };
        }
        let out = run(
            s,
            &const_columns(),
            options(Solver::Auto),
            Policy {
                methods: Methods::none(),
                drop_singletons: true,
            },
            RunControls::default(),
            &team,
        )
        .unwrap();
        assert_eq!(out.sample().n_active(), if frequency { 3 } else { 2 });
        assert_eq!(out.report().singletons, usize::from(!frequency));
        assert!(out
            .report()
            .methods
            .iter()
            .all(|m| !m.requested && m.calls == 0));
        assert_eq!(out.report().work.projection().calls(), 0);
    }
    let (s, c) = ray(&[0., 1., 1., 1.]);
    let out = run(
        s,
        &c,
        Options {
            iterations: 4,
            ..options(Solver::Auto)
        },
        policy(Methods::none()),
        RunControls::default(),
        &team,
    )
    .unwrap();
    assert_eq!(out.sample().n_active(), 4);
    assert_eq!(out.fit(&team).unwrap_err().code, ErrorCode::NonConvergence);
}
#[test]
fn errors_never_return_a_completed_sample_or_a_partial_fit() {
    let team = ThreadTeam::new(1).unwrap();
    let c = const_columns();
    assert_eq!(
        run(
            scalar(&[1., 1.]),
            &c,
            options(Solver::Cmg),
            policy(Methods::none()),
            RunControls::default(),
            &team
        )
        .unwrap_err()
        .code,
        ErrorCode::Unsupported
    );
    let mut malformed = const_columns();
    malformed.roles.clear();
    assert_eq!(
        run(
            scalar(&[1., 1.]),
            &malformed,
            options(Solver::Auto),
            policy(Methods::none()),
            RunControls::default(),
            &team
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidArgument
    );
    let (mut s, c) = ray(&[1., 1., 0., 0.]);
    s.regressors[1] = vec![0., 0., 1., -2.];
    let error = run(
        s,
        &c,
        options(Solver::Auto),
        policy(Methods::only(Method::IteratedRectifier)),
        RunControls {
            ir_iterations: 1,
            ..RunControls::default()
        },
        &team,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert!(error.message.contains("exhausted"));
    let (s, mut c) = ray(&[0., 1., 1.]);
    c.roles[0] = Role::AbsorbedIntercept;
    assert_eq!(
        run(
            s,
            &c,
            options(Solver::Auto),
            policy(Methods::only(Method::IteratedRectifier)),
            RunControls::default(),
            &team
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        run(
            categorical(&[vec![0, 1]], &[0., 1.]),
            &const_columns(),
            options(Solver::Auto),
            Policy {
                drop_singletons: true,
                methods: Methods::default()
            },
            RunControls::default(),
            &team
        )
        .unwrap_err()
        .code,
        ErrorCode::InsufficientObservations
    );
}
#[test]
fn many_restarts_bound_the_public_trace_without_losing_totals_or_digest() {
    let count = 70;
    let n = 2 * count + 4;
    let mut y = vec![1.; n];
    for i in 0..count {
        y[2 * i + 1] = 0.;
    }
    let mut s = scalar(&y);
    let mut recipes = Vec::new();
    let mut roles = Vec::new();
    for i in 0..count {
        let mut x = vec![0.; n];
        x[2 * i + 1] = -1.;
        recipes.push(ColumnRecipe::Continuous {
            source: s.regressors.len(),
        });
        roles.push(Role::Ordinary);
        s.regressors.push(x);
    }
    recipes.push(ColumnRecipe::Intercept);
    roles.push(Role::AbsorbedIntercept);
    let c = columns(recipes, roles);
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    let out = run(
        copy_sample(&s),
        &c,
        options(Solver::Auto),
        policy(Methods::only(Method::Simplex)),
        RunControls::default(),
        &one,
    )
    .unwrap();
    let repeated = run(
        s,
        &c,
        options(Solver::Auto),
        policy(Methods::only(Method::Simplex)),
        RunControls::default(),
        &eight,
    )
    .unwrap();
    assert_eq!(out, repeated);
    assert_eq!(out.sample().n_active(), n - count);
    assert_eq!(out.report().methods[1].primary, count);
    assert!(out.report().events > TRACE_LIMIT);
    assert_eq!(out.report().trace.iter().flatten().count(), TRACE_LIMIT);
    assert!(out.report().methods[1].calls > TRACE_LIMIT);
    assert_eq!(out.report().methods[1].last_generation, Some(count));
    assert_eq!(
        out.report().methods[1].final_status,
        Some(Status::NoCandidates)
    );
}
