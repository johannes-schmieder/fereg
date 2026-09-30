//! Exact full-model rays expose projection errors hidden by separate FE scores.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::{DenseMatrix, Weights},
    fe::{Backend, Design, Engine},
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
                let mut work = Work::default();
                let fit = wls::solve_absorbed_recorded(
                    &mut engine,
                    &raw,
                    &u,
                    &roles,
                    AbsorbedControls {
                        finite: policy,
                        basis: Some(&basis),
                    },
                    &mut work,
                    &team,
                )
                .unwrap_or_else(|e| panic!("seed={seed}, solver={solver:?}: {e}"));
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
