//! A captured tiny FE response has an independently regenerated 100-digit witness.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::{DenseMatrix, Weights},
    fe::{CmgSession, Design, Engine, Plan},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
};
use fereg_oracle::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};
#[path = "support/engine.rs"]
mod inputs;
const WITNESS: &str =
    include_str!("../../../tests/reference/fe_cancellation_decimal_oracle_v1.csv");
struct Fixture {
    effects: Vec<DummyEffect>,
    weights: Vec<f64>,
    response: Vec<f64>,
    residual: Vec<f64>,
}
fn fixture() -> Fixture {
    let mut codes = vec![Vec::new(); 3];
    let mut weights = Vec::new();
    let mut response = Vec::new();
    let mut residual = Vec::new();
    for (row, line) in WITNESS.lines().skip(1).enumerate() {
        let v: Vec<_> = line.split(',').collect();
        assert_eq!(v.len(), 7);
        assert_eq!(v[0].parse::<usize>().unwrap(), row);
        for effect in 0..3 {
            codes[effect].push(v[effect + 1].parse::<u32>().unwrap());
        }
        weights.push(v[4].parse().unwrap());
        response.push(v[5].parse().unwrap());
        residual.push(v[6].parse().unwrap());
    }
    Fixture {
        effects: codes
            .into_iter()
            .map(|c| {
                let levels = 1 + *c.iter().max().unwrap() as usize;
                DummyEffect::new(c, levels, true, vec![]).unwrap()
            })
            .collect(),
        weights,
        response,
        residual,
    }
}
fn norm(values: &[f64], weights: &[f64]) -> f64 {
    values
        .iter()
        .zip(weights)
        .fold(0_f64, |n, (&v, &w)| n.hypot(w.sqrt() * v))
}
fn error(actual: &[f64], expected: &[f64], weights: &[f64]) -> f64 {
    norm(
        &actual
            .iter()
            .zip(expected)
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
        weights,
    ) / norm(expected, weights)
}
fn candidate_evidence(
    label: &str,
    design: &Design<'_>,
    rhs: &DenseMatrix,
    coefficients: &DenseMatrix,
    fixture: &Fixture,
    tolerance: f64,
    team: &ThreadTeam,
) -> fereg_engine::Result<()> {
    let residuals = design.reconstruct(rhs, coefficients, team)?;
    let residual = residuals.column(0)?;
    eprintln!(
        "CMG {label}: witness error={:.17e}, weighted RHS norm={:.17e}, weighted residual norm={:.17e}, certificate={:?}",
        error(&residual, &fixture.residual, &fixture.weights),
        norm(&fixture.response, &fixture.weights),
        norm(&residual, &fixture.weights),
        design.certify(rhs, coefficients, &residuals, tolerance, team),
    );
    Ok(())
}
fn cmg_candidate_evidence(
    design: &Design<'_>,
    rhs: &DenseMatrix,
    fixture: &Fixture,
    options: &Options,
    team: &ThreadTeam,
) -> fereg_engine::Result<()> {
    let plan = Plan::new(design, Estimator::Ppml, options)?;
    let mut session = CmgSession::new(design, plan.cmg_core().unwrap())?;
    let preparation = session.prepare(plan.linear_budget(), plan.tolerance(), team);
    eprintln!("CMG diagnostic setup work={:?}", session.setup_progress());
    preparation?;
    let initial = session.solve(rhs, plan.linear_budget(), plan.tolerance(), team)?;
    eprintln!("CMG initial candidate work={:?}", initial.progress);
    candidate_evidence(
        "initial",
        design,
        rhs,
        &initial.coefficients,
        fixture,
        plan.tolerance(),
        team,
    )?;
    let refined = session.solve_refined(rhs, plan.linear_budget(), plan.tolerance(), team)?;
    eprintln!("CMG refined candidate work={:?}", refined.progress);
    candidate_evidence(
        "refined",
        design,
        rhs,
        &refined.coefficients,
        fixture,
        plan.tolerance(),
        team,
    )?;
    let mut gauged = DenseMatrix::from_columns(&[refined.coefficients.column(0)?])?;
    let normalization = design.normalize_coefficients(&mut gauged, plan.tolerance(), team);
    eprintln!("CMG diagnostic gauge result={normalization:?}");
    normalization?;
    candidate_evidence(
        "gauged",
        design,
        rhs,
        &gauged,
        fixture,
        plan.tolerance(),
        team,
    )
}
#[test]
fn captured_tiny_recovery_retains_acceptance_and_independent_accuracy() {
    let f = fixture();
    assert_eq!(f.response.len(), 206);
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: f.response.clone(),
        regressors: vec![],
        effects: f.effects.clone(),
        weights: OracleWeights::Wls(f.weights.clone()),
        clusters: vec![],
    })
    .unwrap();
    // Dense binary64 QR rounds the large fitted response before subtraction;
    // the high-precision witness has authority over its tiny within contrast.
    assert!(error(&oracle.residuals, &f.residual, &f.weights) <= 1e-9);
    let sample = inputs::prepare(&f.effects, &f.response);
    let design = Design::new(&sample, Weights::Provided(&f.weights)).unwrap();
    let rhs = DenseMatrix::from_columns(&[f.response.clone()]).unwrap();
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let mut reference = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let options = Options {
                solver,
                tolerance: 1e-12,
                ..Options::for_estimator(Estimator::Ppml)
            };
            let mut engine = Engine::new(&design, Estimator::Ppml, &options, &team).unwrap();
            let result = engine.project(&rhs, &team);
            // Observe a fresh unaccepted candidate without changing publication
            // guards; Linux can reject a refinement that settles on this Mac.
            if solver == Solver::Cmg && workers == 1 {
                eprintln!(
                    "CMG candidate diagnostic result={:?}",
                    cmg_candidate_evidence(&design, &rhs, &f, &options, &team),
                );
            }
            // Machine roundoff can change whether a tiny fitted update is
            // accepted. Every success still needs the unchanged independent
            // witness and original score certificate.
            match &result {
                Ok(fit) => {
                    assert!(
                        error(&fit.residuals.column(0).unwrap(), &f.residual, &f.weights) <= 1e-9
                    );
                    assert!(fit.certificate.relative_score[0] <= options.tolerance);
                    if solver == Solver::Cmg {
                        assert!(fit.diagnostics.iter().all(|d| {
                            d.used == fereg_engine::fe::Backend::Cmg && d.fallback.is_none()
                        }));
                    }
                }
                Err(failure) => {
                    assert_ne!(
                        solver,
                        Solver::Cmg,
                        "explicit CMG must recover this fixture: {failure:?}; work={:?}",
                        engine.last_diagnostics()
                    );
                    assert_eq!(failure.code, fereg_engine::ErrorCode::NonConvergence);
                }
            }
            let work = engine.last_diagnostics().to_vec();
            assert!(!work.is_empty());
            assert!(work
                .iter()
                .all(|r| r.requested == solver && r.iterations > 0));
            if let Some(prior) = &reference {
                assert_eq!(prior, &(result, work));
            } else {
                reference = Some((result, work));
            }
        }
    }
}
