use super::*;
use crate::{
    data::DenseMatrix,
    execution::{self, interruption_tests::every_checkpoint, Poll, Probe},
    ppml::separation::{Methods, Policy},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::Role,
};
use std::cell::Cell;

#[test]
fn nullspace_initialization_elimination_and_boundary_images_abort_every_checkpoint() {
    let mut matrix = DenseMatrix::zeros(49, 33).unwrap();
    for row in 0..49 {
        matrix[(row, 0)] = 1.;
        if row > 1 {
            for column in 1..33 {
                matrix[(row, column)] = ((row + column) % 5) as f64 - 2.;
            }
        }
    }
    let positive = [0, 1];
    let boundary: Vec<_> = (2..49).collect();
    let reduce = |poll: Poll<'_>| {
        nullspace::reduce(&matrix, &positive, &boundary, 1e-5, &mut |_| Ok(()), poll)
    };
    let expected = reduce(None).unwrap();
    assert_eq!(expected.basis.len(), 32);
    for (column, basis) in expected.basis.iter().enumerate() {
        for row in positive {
            assert_eq!(matrix.row_combination(row, basis, 0.), 0.);
        }
        for (row, &original) in boundary.iter().enumerate() {
            assert_eq!(
                matrix.row_combination(original, basis, 0.),
                expected.cone[row][column]
            );
        }
    }
    assert!(every_checkpoint(reduce) > 30);
}

fn cone(infeasible: bool, rows: usize, width: usize) -> Vec<Vec<f64>> {
    (0..rows)
        .map(|row| {
            let mut values = vec![0.; width];
            values[0] = if row == rows - 1 {
                0.
            } else if row % 2 == 0 {
                1.
            } else {
                -1.
            };
            if !infeasible {
                values[1] = 1.;
            }
            values
        })
        .collect()
}
#[test]
fn tableau_construction_bland_pivots_outcomes_and_farkas_checks_are_interruptible() {
    for infeasible in [false, true] {
        let cone = cone(infeasible, 47, 31);
        let run = |poll: Poll<'_>| tableau::phase_one_cone(&cone, 100, 1e-5, &mut |_| Ok(()), poll);
        match run(None).unwrap() {
            tableau::PhaseOneConeOutcome::Feasible {
                coordinates,
                iterations,
            } => {
                assert!(!infeasible && iterations > 0);
                let images: Vec<_> = cone
                    .iter()
                    .map(|row| {
                        crate::threads::compensated_sum(
                            row.iter().zip(&coordinates).map(|(a, b)| a * b),
                        )
                    })
                    .collect();
                assert!(images.iter().all(|&v| v >= 0.));
                assert!(images.iter().sum::<f64>() > 0.);
            }
            tableau::PhaseOneConeOutcome::Infeasible {
                dual: multipliers,
                iterations,
            } => {
                assert!(infeasible);
                assert!(iterations <= 100);
                let verify = |poll: Poll<'_>| dual::verify(&cone, &multipliers, 1e-7, poll);
                let certificate = verify(None).unwrap();
                assert_eq!(certificate.stationarity_relative, 0.);
                assert_eq!(certificate.inequality_violation_relative, 0.);
                assert!(certificate.infeasibility_margin_relative > 0.);
                assert!(every_checkpoint(verify) > 3);
            }
        }
        let checkpoints = every_checkpoint(run);
        assert!(checkpoints > if infeasible { 10 } else { 30 });
    }
    let cone = cone(true, 71, 37);
    let mut multipliers = vec![-1.; 72];
    assert!(every_checkpoint(|poll| dual::verify(&cone, &multipliers, 1e-7, poll)) > 5);
    multipliers[0] = -0.5;
    assert_eq!(
        dual::verify(&cone, &multipliers, 1e-7, None)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
}

#[test]
fn cone_normalization_basis_combination_and_observation_images_preserve_exact_arithmetic() {
    let mut basis = vec![vec![0.; 41]; 37];
    for (column, values) in basis.iter_mut().enumerate() {
        values[column] = 1.;
    }
    let mut cone = cone(false, 75, 37);
    for (row, values) in cone.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().enumerate().take(36).skip(2) {
            *value = ((row + column) % 5) as f64 - 2.;
        }
    }
    assert!(
        every_checkpoint(|poll| {
            let (mut cone, mut basis) = (cone.clone(), basis.clone());
            detect::normalize_cone(&mut cone, &mut basis, poll)?;
            Ok((cone, basis))
        }) > 5
    );
    let coordinates = vec![1.; 37];
    assert!(every_checkpoint(|poll| detect::combine_basis(&basis, &coordinates, poll)) > 2);
    let matrix = DenseMatrix::from_columns(&vec![vec![1.; 47]; 37]).unwrap();
    assert!(every_checkpoint(|poll| detect::multiply(&matrix, &coordinates, -1., poll)) > 2);
    assert_eq!(
        detect::multiply(&matrix, &coordinates, -1., None).unwrap(),
        vec![-37.; 47]
    );
}

#[test]
fn dense_model_copy_original_span_verification_and_wide_explicit_columns_are_polled() {
    let matrix = DenseMatrix::from_columns(&[
        (0..2053).map(|row| (row % 7) as f64 - 3.).collect(),
        vec![1.; 2053],
        (0..2053).map(|row| 2. * ((row % 7) as f64 - 3.)).collect(),
    ])
    .unwrap();
    assert!(
        every_checkpoint(|poll| model::span(matrix.clone(), 1e-10, &mut |_| Ok(()), poll)) > 20
    );
    let sample = super::super::numerical_interruption_tests::sample(3, 1, false);
    let ledger = Ledger::new(
        sample,
        Policy {
            methods: Methods::only(Method::Simplex),
            drop_singletons: false,
        },
    )
    .unwrap();
    let raw = DenseMatrix::from_columns(&vec![vec![0.5, 1., 2.]; 2053]).unwrap();
    let roles = vec![Role::Ordinary; 2053];
    assert!(
        every_checkpoint(|poll| {
            let probe = poll.map_or(Probe::default(), |poll| Probe::with_dense(poll, poll));
            let model = Model {
                probe,
                raw: &raw,
                roles: &roles,
                options: Options::for_estimator(Estimator::Ppml),
                controls: Default::default(),
            };
            let design = model::design(ledger.view(), &model)?;
            model::explicit(&design, &model, &mut |_| Ok(()))
        }) > 10
    );
}

fn framed<'a>(raw: &'a DenseMatrix, roles: &'a [Role], probe: Probe<'a>) -> Model<'a> {
    Model {
        probe,
        raw,
        roles,
        options: Options::for_estimator(Estimator::Ppml),
        controls: Default::default(),
    }
}
fn report_checkpoints(infeasible: bool, team: &ThreadTeam) -> usize {
    let (outcome, columns) = if infeasible {
        (
            vec![1., 0., 0., 0., 0.],
            vec![vec![0., 1., -1., 1., -1.], vec![1.; 5]],
        )
    } else {
        (
            vec![1., 0., 0.],
            vec![vec![1., 3., 0.], vec![1., 0., 3.], vec![1.; 3]],
        )
    };
    let mut sample = super::super::numerical_interruption_tests::sample(outcome.len(), 0, false);
    sample.outcome = outcome;
    let ledger = Ledger::new(
        sample,
        Policy {
            methods: Methods::only(Method::Simplex),
            drop_singletons: false,
        },
    )
    .unwrap();
    let before = ledger.trace_digest();
    let raw = DenseMatrix::from_columns(&columns).unwrap();
    let roles = vec![Role::Ordinary; raw.ncols()];
    let expected = ledger
        .view()
        .model_simplex(
            &framed(&raw, &roles, Probe::default()),
            SimplexLimits::default(),
        )
        .unwrap();
    assert_eq!(
        expected.status,
        if infeasible {
            Status::NoSeparation
        } else {
            Status::Certified
        }
    );
    if infeasible {
        assert!(matches!(
            expected.evidence,
            Some(SimplexEvidence::DualInfeasibility { .. })
        ));
    } else {
        assert!(expected.pivots > 0);
    }
    let calls = Cell::new(0);
    let caller = std::thread::current().id();
    let failure = FeregError::new(ErrorCode::NonConvergence, "simplex caller r(430)");
    let run = |stop| {
        calls.set(0);
        execution::run(
            team,
            |notify| {
                let poll = || notify(());
                ledger.view().model_simplex(
                    &framed(&raw, &roles, Probe::with_dense(&poll, &poll)),
                    SimplexLimits::default(),
                )
            },
            &mut |()| {
                assert_eq!(std::thread::current().id(), caller);
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(failure.clone())
                } else {
                    Ok(())
                }
            },
        )
    };
    assert_eq!(run(usize::MAX).unwrap(), expected);
    let total = calls.get();
    assert!(total > 20);
    for stop in 1..=total {
        assert_eq!(run(stop).unwrap_err(), failure);
        assert_eq!(calls.get(), stop);
        assert_eq!(ledger.trace_digest(), before);
        assert_eq!(run(usize::MAX).unwrap(), expected);
        assert_eq!(calls.get(), total);
    }
    total
}
#[test]
fn primal_and_dual_reports_preserve_caller_errors_and_exact_worker_census() {
    for infeasible in [false, true] {
        let one = report_checkpoints(infeasible, &ThreadTeam::new(1).unwrap());
        let eight = report_checkpoints(infeasible, &ThreadTeam::new(8).unwrap());
        assert_eq!(one, eight);
    }
}
