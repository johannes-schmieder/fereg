//! Full-model known rays and literal paired-row impossibility proofs.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::DenseMatrix,
    ppml::separation::*,
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::Role,
};
use fereg_oracle::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};
#[path = "support/engine.rs"]
mod inputs;
#[path = "support/ir_cases.rs"]
mod ir_cases;
fn explicit_effects(effects: &[DummyEffect], rows: &[usize]) -> Vec<DummyEffect> {
    effects
        .iter()
        .map(|effect| {
            DummyEffect::new(
                rows.iter().map(|&r| effect.codes()[r]).collect(),
                effect.n_levels(),
                effect.has_intercept(),
                effect
                    .slopes()
                    .iter()
                    .map(|s| rows.iter().map(|&r| s[r]).collect())
                    .collect(),
            )
            .unwrap()
        })
        .collect()
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        for separated in [false, true] {
            let c = ir_cases::case(seed, separated);
            let mut solvers = vec![Solver::Auto];
            if c.effects.len() >= 2 && c.effects.iter().all(|e| e.slopes().is_empty()) {
                solvers.extend([Solver::Lsmr, Solver::Cmg]);
            }
            for solver in solvers {
                let mut ledger = Ledger::new(
                    inputs::prepare(&c.effects, &c.outcome),
                    Policy {
                        drop_singletons: false,
                        methods: Methods::only(Method::Simplex),
                    },
                )
                .unwrap();
                let zero_rows: Vec<_> = c
                    .outcome
                    .iter()
                    .enumerate()
                    .filter_map(|(r, &y)| (y == 0.).then_some(r))
                    .collect();
                let mut dropped: Vec<usize> = Vec::new();
                for round in 0..=zero_rows.len() {
                    let rows = ledger.view().sample().retained_rows();
                    let columns: Vec<Vec<f64>> = c
                        .columns
                        .iter()
                        .map(|x| rows.iter().map(|&r| x[r]).collect())
                        .collect();
                    let raw = if columns.is_empty() {
                        DenseMatrix::zeros(rows.len(), 0).unwrap()
                    } else {
                        DenseMatrix::from_columns(&columns).unwrap()
                    };
                    let roles = vec![Role::Ordinary; columns.len()];
                    let model = Model {
                        probe: Default::default(),
                        raw: &raw,
                        roles: &roles,
                        controls: Controls::default(),
                        options: Options {
                            solver,
                            ..Options::for_estimator(Estimator::Ppml)
                        },
                    };
                    let r = ledger.view().model_simplex(&model, SimplexLimits::default())
                        .unwrap_or_else(|e| panic!("seed={seed}, solver={solver:?}, separated={separated}, round={round}: {e}"));
                    assert!(r.capacity.explicit_width >= raw.ncols());
                    assert!(r.pivots <= SimplexLimits::default().max_pivots);
                    if r.status != Status::Certified {
                        assert_eq!(
                            r.status,
                            if separated {
                                Status::NoCandidates
                            } else {
                                Status::NoSeparation
                            },
                            "seed={seed}, solver={solver:?}, round={round}"
                        );
                        let terminal = ledger
                            .view()
                            .replay_simplex(&r, &model, SimplexLimits::default())
                            .unwrap();
                        assert_eq!(terminal.generation(), ledger.view().generation());
                        assert_eq!(terminal.pivots(), r.pivots);
                        break;
                    }
                    assert!(
                        separated,
                        "paired complete rows cannot separate: seed={seed}"
                    );
                    let proposal = r.direction.as_ref().unwrap();
                    assert!(!proposal.rows.is_empty());
                    assert!(proposal.rows.iter().all(|row| zero_rows.contains(row)));
                    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
                        outcome: proposal.direction.clone(),
                        regressors: columns,
                        effects: explicit_effects(&c.effects, rows),
                        weights: OracleWeights::Unit,
                        clusters: vec![],
                    })
                    .unwrap();
                    let distance = oracle
                        .residuals
                        .iter()
                        .fold(0_f64, |norm, &v| norm.hypot(v))
                        / proposal
                            .direction
                            .iter()
                            .fold(0_f64, |norm, &v| norm.hypot(v));
                    assert!(
                        distance <= model.controls.projection_tolerance,
                        "seed={seed}, distance={distance:e}"
                    );
                    if seed % 43 == 0 {
                        assert_eq!(
                            r,
                            ledger
                                .view()
                                .model_simplex(&model, SimplexLimits::default())
                                .unwrap()
                        );
                        assert_eq!(
                            ledger
                                .view()
                                .certify_direction(proposal, &model, &one)
                                .unwrap(),
                            ledger
                                .view()
                                .certify_direction(proposal, &model, &eight)
                                .unwrap()
                        );
                    }
                    dropped.extend(&proposal.rows);
                    ledger
                        .accept_direction(proposal, &model, &one)
                        .unwrap_or_else(|e| {
                            panic!("span seed={seed}, solver={solver:?}, round={round}: {e}")
                        });
                }
                if separated {
                    dropped.sort_unstable();
                    assert_eq!(dropped, zero_rows, "seed={seed}, solver={solver:?}");
                    assert!(ledger.view().sample().outcome.iter().all(|&y| y > 0.));
                } else {
                    assert!(dropped.is_empty());
                    for row in (0..c.outcome.len()).step_by(2) {
                        for x in &c.columns {
                            assert_eq!(x[row], x[row + 1]);
                        }
                        for effect in &c.effects {
                            assert_eq!(effect.codes()[row], effect.codes()[row + 1]);
                            for slope in effect.slopes() {
                                assert_eq!(slope[row], slope[row + 1]);
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn categorical_and_sloped_gauges_cannot_turn_paired_rows_into_separation() {
    // Seed 3's redundant categorical design and seed 210's sloped design
    // exposed tiny gauge images that phase-I incorrectly promoted to rays.
    // Literal identical complete rows are the independent impossibility proof.
    campaign(3, 1);
    campaign(210, 1);
}
#[test]
fn full_model_simplex_matches_independent_rays_and_paired_row_proofs() {
    campaign(0, 200);
}
#[test]
#[ignore = "long full-model simplex differential and ledger campaign"]
fn two_thousand_full_model_simplex_designs() {
    campaign(200, 2000);
}
