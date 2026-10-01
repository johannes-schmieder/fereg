use super::*;
use crate::{
    data::{DropReason, Prepared, PreparedAdjustment, PreparedWeights, Weights},
    fe::{Plan, Solver},
    spec::{Estimator, Options},
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};

const WITNESS: &str =
    include_str!("../../../../../tests/reference/fe_cancellation_decimal_oracle_v1.csv");

fn fixture() -> (Prepared, Vec<f64>, Vec<f64>) {
    let mut columns = vec![Vec::new(); 3];
    let mut weights = Vec::new();
    let mut outcome = Vec::new();
    let mut residual = Vec::new();
    for (row, line) in WITNESS.lines().skip(1).enumerate() {
        let values: Vec<_> = line.split(',').collect();
        assert_eq!(values[0].parse::<usize>().unwrap(), row);
        for column in 0..3 {
            columns[column].push(values[column + 1].parse::<f64>().unwrap());
        }
        weights.push(values[4].parse().unwrap());
        outcome.push(values[5].parse().unwrap());
        residual.push(values[6].parse().unwrap());
    }
    let source = |id| SourceId::new(id).unwrap();
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=3)
                .map(|id| SourceEntry {
                    id: source(id),
                    canonical_name: format!("s{}", id - 1),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (1..=3)
            .map(|id| AbsorbedTerm {
                id: AbsorbTermId::new(id).unwrap(),
                categorical_sources: vec![source(id)],
                slope_sources: vec![],
                intercept: true,
                label: format!("g{}", id - 1),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    let n = outcome.len();
    assert_eq!(n, 206);
    let sample = prepare_fixed_effects(
        &model,
        Prepared {
            topology: None,
            n_original: n,
            n_candidate: n,
            original_row_by_active: (0..n).collect(),
            outcome,
            regressors: columns,
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; n],
        },
    )
    .unwrap();
    (sample, weights, residual)
}

fn norm(values: &[f64], weights: &[f64]) -> f64 {
    values
        .iter()
        .zip(weights)
        .fold(0_f64, |n, (&v, &w)| n.hypot(w.sqrt() * v))
}
fn distance(left: &[f64], right: &[f64], weights: &[f64]) -> f64 {
    let differences: Vec<_> = left.iter().zip(right).map(|(a, b)| a - b).collect();
    norm(&differences, weights) / norm(right, weights)
}

#[test]
fn captured_tiny_cmg_refinement_observes_the_exact_stopping_measurements() {
    let (sample, weights, witness) = fixture();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let rhs = DenseMatrix::from_columns(&[sample.outcome.clone()]).unwrap();
    let options = Options {
        solver: Solver::Cmg,
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let plan = Plan::new(&design, Estimator::Ppml, &options).unwrap();
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let mut session = CmgSession::new(&design, plan.cmg_core().unwrap()).unwrap();
        session
            .prepare(plan.linear_budget(), plan.tolerance(), &team)
            .unwrap();
        let expected = session
            .solve_refined(&rhs, plan.linear_budget(), plan.tolerance(), &team)
            .unwrap();
        let mut observed = session
            .solve(&rhs, plan.linear_budget(), plan.tolerance(), &team)
            .unwrap();
        let mut measurements = Vec::new();
        let mut previous_residual = None;
        refinement::run_observed(
            &design,
            &rhs,
            (&mut observed.coefficients, &mut observed.progress),
            refinement::Limits {
                budget: plan.linear_budget(),
                tolerance: plan.tolerance(),
                poll: None,
            },
            &team,
            |residual, remaining, tolerance, team| {
                let correction = session.solve_with_roundoff(
                    residual,
                    refinement::Limits {
                        budget: remaining,
                        tolerance,
                        poll: None,
                    },
                    f64::EPSILON,
                    team,
                )?;
                Ok((correction.coefficients, correction.progress))
            },
            |point, coefficients| {
                let residual = design.reconstruct(&rhs, coefficients, &team).unwrap();
                let residual = residual.column(0).unwrap();
                let applied = previous_residual.as_ref().map(
                    |(previous, within): &(Vec<f64>, f64)| {
                        residual.iter().zip(previous).zip(&weights).fold(
                            0_f64,
                            |norm, ((next, old), weight)| {
                                norm.hypot(weight.sqrt() * (old - next) / within)
                            },
                        )
                    },
                );
                if applied == Some(0.) {
                    assert_eq!(point.update, 0.);
                }
                let forward = distance(&residual, &witness, &weights);
                eprintln!(
                    "CMG refinement workers={workers}: {point:?}, applied residual update={applied:?}, witness error={forward:.17e}"
                );
                previous_residual = Some((residual, point.within_norm));
                measurements.push(point);
            },
        )
        .unwrap();
        assert_eq!(observed, expected, "observer changed the numerical path");
        assert!(measurements.len() > 1);
        assert_eq!(measurements.last().unwrap().update, 0.);
        assert_eq!(
            *reference.get_or_insert((measurements.clone(), expected)),
            (measurements, observed)
        );
        assert!(
            session
                .solve_refined(&rhs, plan.linear_budget(), plan.tolerance(), &team)
                .unwrap()
                .progress
                .iter()
                .all(|p| p.stop == StopReason::Converged),
            "explicit CMG must recover the captured tiny-response fixture"
        );
    }
}
