//! Full-chain sample proofs followed by an independent dense Newton root.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    ppml::separation::*,
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    topology::prepare_implicit_constant,
    wls::Role,
};
use fereg_oracle::{
    explicit_ols::DummyEffect,
    explicit_ppml::{fit_explicit_ppml, PpmlProblem, PpmlResult, PpmlWeights},
};
#[path = "support/engine.rs"]
mod inputs;
#[path = "support/ir_cases.rs"]
mod ir_cases;

struct Fixture {
    case: ir_cases::Case,
    sample: Prepared,
    columns: CompiledColumns,
    policy: Policy,
    controls: RunControls,
}
fn fixture(seed: usize, separated: bool) -> Fixture {
    let mode = (seed / 4) % 5;
    let mut c = ir_cases::case(seed, separated && mode != 4);
    let n = c.outcome.len();
    if separated && mode == 4 {
        // Mu alone is tested on a literal isolated ray. Its nomination
        // heuristic is not a comprehensive separation proof.
        c.outcome.fill(1.);
        c.outcome[seed % n] = 0.;
        let mut ray = vec![0.; n];
        ray[seed % n] = -1.;
        c.columns.push(ray);
    }
    let implicit = c.effects.is_empty();
    let mut sample = inputs::prepare(&c.effects, &c.outcome);
    if implicit {
        sample = prepare_implicit_constant(sample).unwrap();
        assert_eq!(c.columns.remove(0), vec![1.; n]);
        c.effects
            .push(DummyEffect::new(vec![0; n], 1, true, vec![]).unwrap());
    }
    let source = sample.regressors.len();
    sample.regressors.extend(c.columns.clone());
    let mut recipes: Vec<_> = (0..c.columns.len())
        .map(|j| ColumnRecipe::Continuous { source: source + j })
        .collect();
    recipes.push(ColumnRecipe::Intercept);
    let mut roles = vec![Role::Ordinary; c.columns.len()];
    roles.push(Role::AbsorbedIntercept);
    let columns = CompiledColumns {
        slots: (0..recipes.len())
            .map(|i| ColumnSlot {
                name: format!("x{i}"),
                syntax: SyntaxStatus::Active,
                generated: Some(i),
            })
            .collect(),
        constant_slot: Some(c.columns.len()),
        structural_intercept: true,
        implicit_constant: implicit,
        recipes,
        roles,
    };
    sample.weights = match seed % 3 {
        0 => PreparedWeights::None,
        _ => PreparedWeights::Provided {
            kind: if seed % 3 == 1 {
                WeightKind::Frequency
            } else {
                WeightKind::Probability
            },
            values: (0..n).map(|i| (1 + (i / 2) % 5) as f64).collect(),
        },
    };
    let offset: Vec<_> = (0..n).map(|i| (i / 2 % 7) as f64 * 0.04).collect();
    sample.adjustment = match (seed / 3) % 3 {
        0 => PreparedAdjustment::None,
        1 => PreparedAdjustment::Offset(offset),
        _ => PreparedAdjustment::Exposure(offset.iter().map(|v| v.exp()).collect()),
    };
    // Original-axis holes exercise recipe regeneration and all row-scoped
    // compaction independently of the detector's active positions.
    sample.n_original = 2 * n + 3;
    sample.n_candidate = sample.n_original - 1;
    sample.original_row_by_active = (0..n).map(|i| 2 * i + 1).collect();
    sample.drop_reasons = vec![DropReason::MissingOutcome; sample.n_original];
    sample.drop_reasons[0] = DropReason::ExcludedByIfIn;
    for &row in &sample.original_row_by_active {
        sample.drop_reasons[row] = DropReason::Retained;
    }
    let methods = match mode {
        0 | 2 => Methods::default(),
        1 => Methods::parse("mu ir simplex fe").unwrap(),
        3 => Methods::only(Method::IteratedRectifier),
        _ => Methods::only(Method::Mu),
    };
    let mut controls = RunControls::default();
    if mode == 2 {
        controls.simplex.max_cells = 1;
    }
    Fixture {
        case: c,
        sample,
        columns,
        policy: Policy {
            // Singleton cascade behavior has separate exact fixtures; here
            // every retained row is fixed by a full-model ray or pair proof.
            drop_singletons: false,
            methods,
        },
        controls,
    }
}
fn oracle(c: &ir_cases::Case, sample: &Prepared) -> PpmlResult {
    let indices: Vec<_> = sample.retained_rows().iter().map(|r| (r - 1) / 2).collect();
    let mut columns: Vec<Vec<f64>> = c
        .columns
        .iter()
        .map(|x| indices.iter().map(|&i| x[i]).collect())
        .collect();
    for effect in &c.effects {
        for level in 0..effect.n_levels() {
            for channel in effect
                .has_intercept()
                .then(|| vec![1.; c.outcome.len()])
                .into_iter()
                .chain(effect.slopes().iter().cloned())
            {
                columns.push(
                    indices
                        .iter()
                        .map(|&i| {
                            if effect.codes()[i] as usize == level {
                                channel[i]
                            } else {
                                0.
                            }
                        })
                        .collect(),
                );
            }
        }
    }
    let weights = match &sample.weights {
        PreparedWeights::None => PpmlWeights::Unit,
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values,
        } => PpmlWeights::Frequency(values.iter().map(|v| *v as u32).collect()),
        PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values,
        } => PpmlWeights::Probability(values.clone()),
        _ => unreachable!("PPML campaign uses only supported weight kinds"),
    };
    let offset = match &sample.adjustment {
        PreparedAdjustment::None => vec![],
        PreparedAdjustment::Offset(v) => v.clone(),
        PreparedAdjustment::Exposure(v) => v.iter().map(|v| v.ln()).collect(),
    };
    fit_explicit_ppml(&PpmlProblem {
        design: (0..indices.len())
            .map(|row| columns.iter().map(|x| x[row]).collect())
            .collect(),
        outcome: sample.outcome.clone(),
        offset,
        weights,
        clusters: vec![],
    })
    .unwrap()
}
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "seed={seed}: {a:.17e} != {b:.17e}"
    );
}
fn check_compaction(sample: &Prepared, f: &Fixture, indices: &[usize]) {
    let selected = |values: &[f64]| indices.iter().map(|&i| values[i]).collect::<Vec<_>>();
    assert_eq!(sample.outcome, selected(&f.case.outcome));
    for (original, compacted) in f.sample.regressors.iter().zip(&sample.regressors) {
        assert_eq!(*compacted, selected(original));
    }
    match (&f.sample.weights, &sample.weights) {
        (PreparedWeights::None, PreparedWeights::None) => (),
        (
            PreparedWeights::Provided { kind, values },
            PreparedWeights::Provided {
                kind: compact_kind,
                values: compact_values,
            },
        ) => {
            assert_eq!(kind, compact_kind);
            assert_eq!(*compact_values, selected(values));
        }
        _ => panic!("weight kind changed during separation"),
    }
    match (&f.sample.adjustment, &sample.adjustment) {
        (PreparedAdjustment::None, PreparedAdjustment::None) => (),
        (PreparedAdjustment::Offset(v), PreparedAdjustment::Offset(compact))
        | (PreparedAdjustment::Exposure(v), PreparedAdjustment::Exposure(compact)) => {
            assert_eq!(*compact, selected(v));
        }
        _ => panic!("adjustment kind changed during separation"),
    }
}
fn check_sample(out: &FinalSample, f: &Fixture, separated: bool, seed: usize) {
    let expected: Vec<_> = f
        .case
        .outcome
        .iter()
        .enumerate()
        .filter_map(|(i, &y)| (!separated || y > 0.).then_some(2 * i + 1))
        .collect();
    assert_eq!(out.sample().retained_rows(), expected, "seed={seed}");
    assert_eq!(out.roles(), f.columns.roles);
    assert_eq!(out.raw(), &f.columns.generate(out.sample()).unwrap());
    let indices: Vec<_> = expected.iter().map(|r| (r - 1) / 2).collect();
    check_compaction(out.sample(), f, &indices);
    for (j, column) in f.case.columns.iter().enumerate() {
        assert_eq!(
            out.raw().column(j).unwrap(),
            indices.iter().map(|&i| column[i]).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        out.raw().column(f.case.columns.len()).unwrap(),
        vec![1.; indices.len()]
    );
    assert_eq!(out.sample().drop_reasons[0], DropReason::ExcludedByIfIn);
    for row in (2..out.sample().n_original).step_by(2) {
        assert_eq!(out.sample().drop_reasons[row], DropReason::MissingOutcome);
    }
    let r = out.report();
    assert_eq!(r.rounds, r.generation + 1);
    assert_eq!(r.singletons, 0);
    assert!(r.methods.iter().all(|m| m.requested || m.calls == 0));
    for method in f.policy.methods.iter() {
        let m = r.methods[method as usize];
        assert!(m.requested);
        assert_eq!(m.last_generation, Some(r.generation));
        assert!(matches!(
            m.final_status,
            Some(Status::NoCandidates | Status::NoSeparation | Status::SkippedCapacity)
        ));
        assert!(m.calls > 0);
        if method != Method::FixedEffects {
            assert!(m.replays > 0);
        }
    }
    assert_eq!(
        r.methods.iter().map(|m| m.primary).sum::<usize>() + r.exact_followup,
        f.case.outcome.len() - indices.len()
    );
    if (seed / 4) % 5 == 2 && !separated {
        assert_eq!(r.capacity_resolutions, 1);
        assert_eq!(r.methods[1].final_status, Some(Status::SkippedCapacity));
    }
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        for separated in [false, true] {
            let f = fixture(seed, separated);
            let mut solvers = vec![Solver::Auto];
            if f.case.effects.len() >= 2 && f.case.effects.iter().all(|e| e.slopes().is_empty()) {
                solvers.extend([Solver::Lsmr, Solver::Cmg]);
            }
            for solver in solvers {
                let options = Options {
                    solver,
                    ..Options::for_estimator(Estimator::Ppml)
                };
                let fresh = fixture(seed, separated);
                let out = run(
                    fresh.sample,
                    &f.columns,
                    options,
                    f.policy,
                    f.controls,
                    &one,
                )
                .unwrap_or_else(|e| panic!("chain seed={seed}, {separated}, {solver:?}: {e}"));
                check_sample(&out, &f, separated, seed);
                let expected = oracle(&f.case, out.sample());
                let fit = out
                    .fit(&one)
                    .unwrap_or_else(|e| panic!("root seed={seed}, {separated}, {solver:?}: {e}"));
                close(fit.evaluation.objective.deviance, expected.deviance, seed);
                for row in 0..out.sample().n_active() {
                    close(fit.point.eta[row], expected.eta[row], seed);
                    close(fit.evaluation.mean[row], expected.mean[row], seed);
                }
                assert!(fit.scores.passed);
                assert!(fit.diagnostics.strict_checks >= 2);
                if seed % 41 == 0 {
                    let fresh = fixture(seed, separated);
                    let repeated = run(
                        fresh.sample,
                        &f.columns,
                        options,
                        f.policy,
                        f.controls,
                        &eight,
                    )
                    .unwrap();
                    assert_eq!(out, repeated);
                    assert_eq!(fit, repeated.fit(&eight).unwrap());
                }
            }
        }
    }
}
#[test]
fn explicit_lsmr_chain_preserves_the_strict_boundary_frame() {
    let f = fixture(634, true);
    let raw = f.columns.generate(&f.sample).unwrap();
    let one = ThreadTeam::new(1).unwrap();
    let design = fereg_engine::fe::Design::new(&f.sample, Weights::Unit).unwrap();
    let mut engine = fereg_engine::fe::Engine::new(
        &design,
        Estimator::Ppml,
        &Options {
            solver: Solver::Lsmr,
            tolerance: 1e-12,
            ..Options::for_estimator(Estimator::Ppml)
        },
        &one,
    )
    .unwrap();
    let span = fereg_engine::wls::solve_absorbed(
        &mut engine,
        &raw,
        &vec![0.; f.sample.n_active()],
        &f.columns.roles,
        fereg_engine::wls::Policy {
            fe_collinearity: f64::EPSILON.powi(2),
            ..fereg_engine::wls::Policy::default()
        },
        &one,
    )
    .unwrap();
    assert_eq!(
        span.finite.statuses.last(),
        Some(&fereg_engine::wls::Status::AbsorbedInterceptDeferred)
    );
    assert_eq!(raw.column(raw.ncols() - 1).unwrap(), vec![1.; raw.nrows()]);
    assert!(span
        .within
        .column(raw.ncols() - 1)
        .unwrap()
        .iter()
        .all(|v| v.abs() <= 1e-12));
    campaign(634, 1);
}
#[test]
fn canonical_chain_and_cold_final_root_match_independent_proofs() {
    campaign(0, 200);
}
#[test]
#[ignore = "long separation coordinator and independent Newton campaign"]
fn two_thousand_complete_separation_chains() {
    campaign(200, 2000);
}
