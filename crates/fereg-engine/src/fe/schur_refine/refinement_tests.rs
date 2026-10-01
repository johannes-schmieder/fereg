use super::*;
use crate::{
    data::{DropReason, Prepared, PreparedAdjustment, PreparedWeights, Weights},
    fe::{Design, Plan},
    spec::{Estimator, Options},
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};
use std::cell::Cell;

const WITNESS: &str =
    include_str!("../../../../../tests/reference/fe_boundary_response_decimal_oracle_v1.csv");
fn fixture() -> (Prepared, Vec<f64>, DenseMatrix, Vec<f64>) {
    let mut sources = vec![Vec::new(); 6];
    let mut weights = Vec::new();
    let mut rhs = vec![Vec::new(); 4];
    let mut residual = Vec::new();
    for (row, line) in WITNESS.lines().skip(1).enumerate() {
        let values: Vec<_> = line.split(',').collect();
        assert_eq!(values.len(), 13);
        assert_eq!(values[0].parse::<usize>().unwrap(), row);
        for column in 0..6 {
            sources[column].push(values[column + 1].parse::<f64>().unwrap());
        }
        weights.push(values[7].parse().unwrap());
        for column in 0..4 {
            rhs[column].push(values[column + 8].parse().unwrap());
        }
        residual.push(values[12].parse().unwrap());
    }
    let source = |id| SourceId::new(id).unwrap();
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=6)
                .map(|id| SourceEntry {
                    id: source(id),
                    canonical_name: format!("s{}", id - 1),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (0..3)
            .map(|index| AbsorbedTerm {
                id: AbsorbTermId::new(index + 1).unwrap(),
                categorical_sources: vec![source(2 * index + 1)],
                slope_sources: vec![source(2 * index + 2)],
                intercept: true,
                label: format!("g{index}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    let n = rhs[0].len();
    assert_eq!(n, 79);
    let sample = prepare_fixed_effects(
        &model,
        Prepared {
            topology: None,
            n_original: n,
            n_candidate: n,
            original_row_by_active: (0..n).collect(),
            outcome: rhs[0].clone(),
            regressors: sources,
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; n],
        },
    )
    .unwrap();
    (
        sample,
        weights,
        DenseMatrix::from_columns(&rhs).unwrap(),
        residual,
    )
}
fn response_evidence(
    label: &str,
    design: &Design<'_>,
    rhs: &DenseMatrix,
    coefficients: &DenseMatrix,
    witness: &[f64],
    team: &ThreadTeam,
) -> (Vec<f64>, bool) {
    let residuals = design.reconstruct(rhs, coefficients, team).unwrap();
    let residual = residuals.column(0).unwrap();
    let error = residual
        .iter()
        .zip(witness)
        .fold(0_f64, |error, (a, b)| error.max((a - b).abs()));
    let coefficient_max = coefficients
        .as_slice()
        .iter()
        .fold(0_f64, |v, x| v.max(x.abs()));
    let certificate = design.certify(rhs, coefficients, &residuals, 1e-10, team);
    eprintln!("Schur response {label}: forward_max={error:.17e}, row50={:.17e}, coefficient_max={coefficient_max:.17e}, certificate={certificate:?}", residual[50]);
    (residual, certificate.is_ok())
}

#[test]
fn captured_boundary_response_observes_refinement_and_retains_forward_accuracy() {
    let (sample, weights, rhs, witness) = fixture();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let options = Options {
        tolerance: 1e-10,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let plan = Plan::new(&design, Estimator::Ppml, &options).unwrap();
    let original = DenseMatrix::from_columns(&[rhs.column(0).unwrap()]).unwrap();
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let mut default = SchurSession::new(&design, &plan, &team).unwrap();
        let expected = default
            .solve_refined(&rhs, MonitorControl::Automatic, &team)
            .unwrap();
        let mut session = SchurSession::new(&design, &plan, &team).unwrap();
        let mut observed = session
            .solve_monitored(&rhs, MonitorControl::Automatic, &team)
            .unwrap();
        eprintln!(
            "Schur boundary workers={workers}: initial={:?}",
            observed.progress
        );
        let active = Cell::new(usize::MAX);
        let mut measurements = Vec::new();
        refinement::run_observed(
            &design, &rhs, (&mut observed.coefficients, &mut observed.progress),
            refinement::Limits { budget: plan.linear_budget(), tolerance: plan.tolerance(), poll: None },
            &team,
            |residual, remaining, tolerance, team| {
                let correction = session.solve_budget(residual, MonitorControl::Automatic, false,
                    refinement::Limits { budget: remaining, tolerance, poll: None }, team)?;
                eprintln!("Schur correction workers={workers}, column={}, tolerance={tolerance:.17e}, remaining={remaining}, work={:?}", active.get(), correction.progress);
                Ok((correction.coefficients, correction.progress))
            },
            |point, coefficients| {
                active.set(point.column);
                eprintln!("Schur guard workers={workers}: {point:?}");
                if point.column == 0 {
                    response_evidence("guard", &design, &original, coefficients, &witness, &team);
                }
                measurements.push(point);
            },
        ).unwrap();
        observed.residuals = design
            .reconstruct(&rhs, &observed.coefficients, &team)
            .unwrap();
        assert_eq!(observed, expected, "observer changed the numerical path");
        assert_eq!(
            *reference.get_or_insert((measurements.clone(), expected)),
            (measurements, observed)
        );
        let (points, candidate) = reference.as_ref().unwrap();
        assert!(!points.is_empty());
        let mut coefficients =
            DenseMatrix::from_columns(&[candidate.coefficients.column(0).unwrap()]).unwrap();
        let mut gauge_evidence = Vec::new();
        for gauge in 0..3 {
            let label = format!("workers={workers}/gauge={gauge}");
            let (residual, certified) =
                response_evidence(&label, &design, &original, &coefficients, &witness, &team);
            let error = residual
                .iter()
                .zip(&witness)
                .fold(0_f64, |v, (a, b)| v.max((a - b).abs()));
            gauge_evidence.push((gauge, error, certified));
            design
                .normalize_coefficients(&mut coefficients, 1e-10, &team)
                .unwrap();
        }
        if candidate.progress[0].stop == StopReason::Converged {
            assert!(
                gauge_evidence
                    .iter()
                    .all(|(_, error, certified)| !certified || *error <= 1e-9),
                "certified Schur response exceeds unchanged forward bound: {gauge_evidence:?}"
            );
        }
    }
}
