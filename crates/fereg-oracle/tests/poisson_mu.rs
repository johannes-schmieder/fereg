//! Isolated full-model rays and paired-row impossibility, independently witnessed.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::{DenseMatrix, PreparedAdjustment, PreparedWeights, WeightKind},
    ppml::{irls::MuStop, separation::*},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::Role,
};
use fereg_oracle::explicit_ols::{fit_explicit_ols, ExplicitOlsProblem, OracleWeights};
#[path = "support/engine.rs"]
mod inputs;
#[path = "support/ir_cases.rs"]
mod ir_cases;
struct Fixture {
    case: ir_cases::Case,
    ledger: Ledger,
    raw: DenseMatrix,
    roles: Vec<Role>,
    boundary: usize,
}
fn fixture(seed: usize, separated: bool) -> Fixture {
    let mut c = ir_cases::case(seed, false);
    let n = c.outcome.len();
    let boundary = seed % n;
    if separated {
        // The literal extra column isolates exactly one zero. Other
        // finite, FE and slope channels cannot invalidate this ray.
        c.outcome.fill(1.);
        c.outcome[boundary] = 0.;
        let mut ray = vec![0.; n];
        ray[boundary] = -1.;
        c.columns.push(ray);
    }
    let mut sample = inputs::prepare(&c.effects, &c.outcome);
    let values = (0..n).map(|r| (1 + (r / 2) % 5) as f64).collect();
    sample.weights = match seed % 3 {
        0 => PreparedWeights::None,
        _ => PreparedWeights::Provided {
            kind: if seed % 3 == 1 {
                WeightKind::Frequency
            } else {
                WeightKind::Probability
            },
            values,
        },
    };
    // Adjustments and prior weights belong to the fitted trajectory,
    // while their values are absent from the homogeneous cone proof.
    let offset: Vec<_> = (0..n).map(|r| (r / 2 % 7) as f64 * 0.04).collect();
    sample.adjustment = match seed % 3 {
        0 => PreparedAdjustment::None,
        1 => PreparedAdjustment::Offset(offset),
        _ => PreparedAdjustment::Exposure(offset.iter().map(|v| v.exp()).collect()),
    };
    let raw = if c.columns.is_empty() {
        DenseMatrix::zeros(n, 0).unwrap()
    } else {
        DenseMatrix::from_columns(&c.columns).unwrap()
    };
    let roles = vec![Role::Ordinary; c.columns.len()];
    let ledger = Ledger::new(
        sample,
        Policy {
            drop_singletons: false,
            methods: Methods::only(Method::Mu),
        },
    )
    .unwrap();
    Fixture {
        case: c,
        ledger,
        raw,
        roles,
        boundary,
    }
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        for separated in [false, true] {
            let fixture = fixture(seed, separated);
            let c = &fixture.case;
            let l = &fixture.ledger;
            let n = c.outcome.len();
            let boundary = fixture.boundary;
            let raw = &fixture.raw;
            let roles = &fixture.roles;
            let mut solvers = vec![Solver::Auto];
            if c.effects.len() >= 2 && c.effects.iter().all(|e| e.slopes().is_empty()) {
                solvers.extend([Solver::Lsmr, Solver::Cmg]);
            }
            for solver in solvers {
                let model = Model {
                    raw,
                    roles,
                    controls: Controls::default(),
                    options: Options {
                        solver,
                        ..Options::for_estimator(Estimator::Ppml)
                    },
                };
                let r = l
                    .view()
                    .fitted_mu(&model, DEFAULT_MU_TOLERANCE, &one)
                    .unwrap_or_else(|e| {
                        panic!("seed={seed}, separated={separated}, solver={solver:?}: {e}")
                    });
                assert_eq!(
                    r.work.projection().requested(solver),
                    r.work.projection().counts().right_hand_sides
                );
                assert!(r.trajectory.accepted_iterations > 0);
                assert!(r.trajectory.certified_linear_frames >= r.trajectory.accepted_iterations);
                assert!(
                    r.trajectory.maximum_deviance_increase_relative <= 1e-12 + 64. * f64::EPSILON
                );
                assert!(r.trajectory.mean_reconstruction_relative <= 64. * f64::EPSILON);
                assert_eq!(
                    r.work.projection().calls(),
                    r.trajectory_diagnostics.linear_frames + r.projection_frames
                );
                if separated {
                    assert_eq!(
                        r.status,
                        Status::Certified,
                        "seed={seed}, solver={solver:?}"
                    );
                    assert_eq!(r.trajectory.terminal, MuStop::CandidateThresholdReached);
                    let p = r.direction.as_ref().unwrap();
                    assert_eq!(p.rows, [boundary]);
                    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
                        outcome: p.direction.clone(),
                        regressors: c.columns.clone(),
                        effects: c.effects.clone(),
                        weights: OracleWeights::Unit,
                        clusters: vec![],
                    })
                    .unwrap();
                    let distance = oracle.residuals.iter().fold(0_f64, |n, &v| n.hypot(v))
                        / p.direction.iter().fold(0_f64, |n, &v| n.hypot(v));
                    assert!(
                        distance <= model.controls.projection_tolerance,
                        "seed={seed}, distance={distance:e}"
                    );
                    l.view()
                        .certify_direction(p, &model, &one)
                        .unwrap_or_else(|e| panic!("span seed={seed}, solver={solver:?}: {e}"));
                } else {
                    assert_eq!(
                        r.status,
                        Status::NoCandidates,
                        "seed={seed}, solver={solver:?}"
                    );
                    assert_eq!(r.candidates, 0);
                    l.view()
                        .replay_mu(&r, &model, DEFAULT_MU_TOLERANCE, &one)
                        .unwrap();
                    // Every zero has a positive partner with identical full
                    // model and adjustment: their fitted means must coincide.
                    for row in (0..n).step_by(2) {
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
                if seed % 43 == 0 {
                    assert_eq!(
                        r,
                        l.view()
                            .fitted_mu(&model, DEFAULT_MU_TOLERANCE, &eight)
                            .unwrap()
                    );
                    if let Some(p) = &r.direction {
                        assert_eq!(
                            l.view().certify_direction(p, &model, &one).unwrap(),
                            l.view().certify_direction(p, &model, &eight).unwrap()
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn fitted_mu_preserves_the_weak_categorical_boundary_frame() {
    for seed in [111, 207, 351, 1047] {
        campaign(seed, 1);
    }
}
#[test]
fn fitted_mu_matches_full_model_rays_and_paired_row_proofs() {
    campaign(0, 200);
}
#[test]
#[ignore = "long fitted-mean trajectory, full-span and replay campaign"]
fn two_thousand_fitted_mu_designs() {
    campaign(200, 2000);
}
