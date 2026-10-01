//! Exact full-model rays expose projection errors hidden by separate FE scores.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::{DenseMatrix, Weights},
    fe::{Backend, Design, Engine, MonitorControl, Plan, SchurSession},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::{self, AbsorbedControls, FrozenBasis, Role, Work},
};
#[path = "support/engine.rs"]
mod inputs;
#[path = "support/ir_cases.rs"]
mod ir_cases;

#[test]
fn separately_certified_frames_retain_exact_full_model_rays_at_boundary_weights() {
    for seed in [55, 155, 1523] {
        let c = ir_cases::case(seed, true);
        let sample = inputs::prepare(&c.effects, &c.outcome);
        let n = c.outcome.len();
        let raw = DenseMatrix::from_columns(&c.columns).unwrap();
        let roles = vec![Role::Ordinary; c.columns.len()];
        let u: Vec<_> = c.outcome.iter().map(|&y| f64::from(y == 0.)).collect();
        let boundary = u.iter().filter(|&&v| v == 1.).count();
        let h: Vec<_> = u
            .iter()
            .map(|&v| {
                if v == 1. {
                    1e-5_f64.powi(2) / boundary as f64
                } else {
                    1.
                }
            })
            .collect();
        let unit = Design::new(&sample, Weights::Unit).unwrap();
        let one = ThreadTeam::new(1).unwrap();
        let mut unit_engine = Engine::new(
            &unit,
            Estimator::Ppml,
            &Options {
                tolerance: 1e-12,
                ..Options::for_estimator(Estimator::Ppml)
            },
            &one,
        )
        .unwrap();
        let policy = wls::Policy {
            score_tolerance: 1e-10,
            ..wls::Policy::default()
        };
        let canonical =
            wls::solve_absorbed(&mut unit_engine, &raw, &vec![0.; n], &roles, policy, &one)
                .unwrap();
        let basis = FrozenBasis::from_fit(&canonical.finite).unwrap();
        let design = Design::new(&sample, Weights::Provided(&h)).unwrap();
        let mut solvers = vec![Solver::Auto];
        if c.effects.iter().all(|e| e.slopes().is_empty()) {
            solvers.extend([Solver::Lsmr, Solver::Cmg]);
        }
        for solver in solvers {
            let options = Options {
                solver,
                tolerance: 1e-10,
                ..Options::for_estimator(Estimator::Ppml)
            };
            let mut reference = None;
            for workers in [1, 2, 4, 8] {
                let team = ThreadTeam::new(workers).unwrap();
                let mut engine = Engine::new(&design, Estimator::Ppml, &options, &team).unwrap();
                if seed == 155 && workers == 1 && solver == Solver::Auto {
                    schur_evidence(&design, &options, &raw, &u, &team);
                }
                let mut work = Work::default();
                let fit = wls::solve_absorbed_recorded(
                    &mut engine,
                    &raw,
                    &u,
                    &roles,
                    AbsorbedControls {
                        probe: Default::default(),
                        finite: policy,
                        basis: Some(&basis),
                    },
                    &mut work,
                    &team,
                )
                .unwrap_or_else(|e| {
                    panic!(
                        "seed={seed}, solver={solver:?}: {e}; reports={:?}, work={:?}, projection_work={:?}, recovery_work={:?}",
                        engine.last_diagnostics(), work.counts(),
                        work.projection().counts(), work.recovery().counts()
                    )
                });
                frame_evidence(seed, solver, workers, &fit, &design, &work);
                assert!(basis.matches(&fit.finite));
                for (column, &beta) in fit.finite.coefficients.iter().enumerate() {
                    let expected = if column + 1 == raw.ncols() { -1. } else { 0. };
                    assert!(
                        (beta - expected).abs() <= 1e-9,
                        "seed={seed}, solver={solver:?}, column={column}, beta={beta:e}"
                    );
                }
                for &residual in &fit.residuals {
                    assert!(
                        residual.abs() <= 1e-9,
                        "seed={seed}, solver={solver:?}, residual={residual:e}"
                    );
                }
                assert!(fit.relative_original_finite_score <= policy.score_tolerance);
                assert!(fit.relative_residual_reconstruction <= policy.score_tolerance);
                assert!(work.projection().counts().refinements > 0);
                assert!(work.counts().maximum_rhs_iterations <= 16_000);
                let expected = match solver {
                    Solver::Auto => Backend::SchurPcg,
                    Solver::Lsmr => Backend::Lsmr,
                    Solver::Cmg => Backend::Cmg,
                };
                assert!(fit
                    .projection_diagnostics
                    .iter()
                    .all(|d| d.requested == solver && d.selected == expected));
                if solver == Solver::Cmg {
                    assert!(fit.projection_diagnostics.iter().all(|d| {
                        d.used == Backend::Cmg && d.fallback.is_none() && d.attempts.len() == 1
                    }));
                }
                if let Some((prior, prior_work)) = &reference {
                    assert_eq!(prior, &fit);
                    assert_eq!(prior_work, &work);
                } else {
                    reference = Some((fit, work));
                }
            }
        }
    }
}

// Independent public candidates localize projection versus combined-model
// recovery without influencing the engine used by the authoritative assertions.
fn schur_evidence(
    design: &Design<'_>,
    options: &Options,
    raw: &DenseMatrix,
    outcome: &[f64],
    team: &ThreadTeam,
) {
    let mut columns = vec![outcome.to_vec()];
    for column in 0..raw.ncols() {
        columns.push(raw.column(column).unwrap());
    }
    let rhs = DenseMatrix::from_columns(&columns).unwrap();
    let plan = Plan::new(design, Estimator::Ppml, options).unwrap();
    for refined in [false, true] {
        let mut session = SchurSession::new(design, &plan, team).unwrap();
        let candidate = if refined {
            session.solve_refined(&rhs, MonitorControl::Automatic, team)
        } else {
            session.solve_monitored(&rhs, MonitorControl::Automatic, team)
        };
        match candidate {
            Ok(candidate) => {
                let mut ray_max = 0_f64;
                let mut ray_norm = 0_f64;
                for row in 0..rhs.nrows() {
                    // u plus the last raw column is a known complete FE ray.
                    let value =
                        candidate.residuals[(row, 0)] + candidate.residuals[(row, raw.ncols())];
                    ray_max = ray_max.max(value.abs());
                    ray_norm = ray_norm.hypot(design.weights().at(row).sqrt() * value);
                }
                let certificate = design.certify(
                    &rhs,
                    &candidate.coefficients,
                    &candidate.residuals,
                    options.tolerance,
                    team,
                );
                eprintln!(
                    "seed=155 Schur refined={refined}: ray_max={ray_max:.17e}, weighted_ray_norm={ray_norm:.17e}, progress={:?}, original_certificate={certificate:?}",
                    candidate.progress
                );
            }
            Err(error) => eprintln!("seed=155 Schur refined={refined}: {error}"),
        }
    }
}

fn frame_evidence(
    seed: usize,
    solver: Solver,
    workers: usize,
    fit: &wls::AbsorbedFit,
    design: &Design<'_>,
    work: &Work,
) {
    let mut residual_max = 0_f64;
    let mut worst_row = 0;
    let mut weighted_norm = 0_f64;
    for (row, &residual) in fit.residuals.iter().enumerate() {
        if residual.abs() > residual_max {
            residual_max = residual.abs();
            worst_row = row;
        }
        weighted_norm = weighted_norm.hypot(design.weights().at(row).sqrt() * residual);
    }
    eprintln!(
        "frame seed={seed}, solver={solver:?}, workers={workers}: beta={:?}, residual_max={residual_max:.17e}, worst_row={worst_row}, worst_weight={:.17e}, weighted_residual_norm={weighted_norm:.17e}, original_finite_score={:.17e}, reconstruction={:.17e}, model_refinements={}, projection={:?}, model={:?}, reports={:?}, recovery={:?}, work={:?}, projection_work={:?}, recovery_work={:?}",
        fit.finite.coefficients, design.weights().at(worst_row),
        fit.relative_original_finite_score, fit.relative_residual_reconstruction,
        fit.model_refinements, fit.projection_certificate, fit.model_certificate,
        fit.projection_diagnostics, fit.recovery_diagnostics,
        work.counts(), work.projection().counts(), work.recovery().counts(),
    );
}
