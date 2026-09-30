use super::*;
use crate::{
    data::{ColumnRecipe, ColumnSlot, PreparedWeights, SyntaxStatus, WeightKind, Weights},
    execution::interruption_tests::every_checkpoint,
    spec::Estimator,
    threads::ThreadTeam,
    wls::{self, Policy, Problem, Role, Slot},
};
use std::{cell::Cell, rc::Rc};

fn columns(constant: bool) -> CompiledColumns {
    let mut result = CompiledColumns {
        slots: vec![
            ColumnSlot {
                name: "base".into(),
                syntax: SyntaxStatus::Base,
                generated: None
            };
            2053
        ],
        recipes: vec![ColumnRecipe::Continuous { source: 0 }; 2],
        roles: vec![Role::Ordinary; 2],
        constant_slot: None,
        structural_intercept: true,
        implicit_constant: true,
    };
    for i in 0..2 {
        result.slots.push(ColumnSlot {
            name: format!("x{i}"),
            syntax: SyntaxStatus::Active,
            generated: Some(i),
        });
    }
    result.slots.push(ColumnSlot {
        name: "omitted".into(),
        syntax: SyntaxStatus::ExplicitlyOmitted,
        generated: None,
    });
    if constant {
        result.constant_slot = Some(result.slots.len());
        result.slots.push(ColumnSlot {
            name: "_cons".into(),
            syntax: SyntaxStatus::Active,
            generated: Some(2),
        });
        result.roles.push(Role::AbsorbedIntercept);
        result.recipes.push(ColumnRecipe::Intercept);
    }
    result
}

#[test]
fn canonical_mapping_and_constant_normalization_cancel_every_checkpoint() {
    let n = 2053;
    let x: Vec<_> = (0..n).map(|i| (i % 19) as f64 / 4. - 1.).collect();
    let y: Vec<_> = x.iter().map(|x| 3. + 2. * x).collect();
    let raw_weights = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: (0..n).map(|i| (1 + i % 3) as f64).collect(),
    };
    let q = ReferenceWeights::new(&raw_weights, n, Estimator::Ols).unwrap();
    let mx = q.mean(&x).unwrap();
    let my = q.mean(&y).unwrap();
    let wy: Vec<_> = y.iter().map(|y| y - my).collect();
    for constant in [false, true] {
        let columns = columns(constant);
        let mut rc = vec![x.clone(), x.clone()];
        let mut wc = vec![x.iter().map(|x| x - mx).collect::<Vec<_>>(); 2];
        if constant {
            rc.push(vec![1.; n]);
            wc.push(vec![0.; n]);
        }
        let raw = DenseMatrix::from_columns(&rc).unwrap();
        let within = DenseMatrix::from_columns(&wc).unwrap();
        let slots: Vec<_> = columns
            .roles
            .iter()
            .map(|&role| Slot {
                role,
                reference_norm: 1000.,
            })
            .collect();
        let mut reference = None;
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let fit = wls::solve(
                &Problem {
                    design: &within,
                    outcome: &wy,
                    weights: Weights::Reference(q),
                    slots: &slots,
                    has_absorbed_intercept: true,
                },
                Policy::default(),
                &team,
            )
            .unwrap();
            let count = team.install(|| {
                every_checkpoint(|poll| report_polled(&columns, &raw, &within, &y, q, &fit, poll))
            });
            let actual = report(&columns, &raw, &within, &y, q, &fit).unwrap();
            assert_eq!(actual.finite_rank, 1);
            assert_eq!(actual.statuses[2053], Status::Kept);
            assert_eq!(actual.statuses[2054], Status::NumericallyRankDeficient);
            assert_eq!(actual.statuses[2055], Status::ExplicitlyOmitted);
            assert!((actual.coefficients[2053] - 2.).abs() < 1e-12);
            if constant {
                assert!((actual.reported_constant - 3.).abs() < 1e-12);
            }
            assert_eq!(
                *reference.get_or_insert((actual, count)),
                (report(&columns, &raw, &within, &y, q, &fit).unwrap(), count)
            );
        }
    }
}

#[test]
fn nested_mean_poll_runs_on_the_caller_and_retains_host_error_identity() {
    let q = ReferenceWeights::new(&PreparedWeights::None, 3, Estimator::Ols).unwrap();
    let values = vec![0.25; 4099];
    let original = FeregError::new(ErrorCode::NonConvergence, "host reporting r(430)");
    let caller = std::thread::current().id();
    let seen = Rc::new(Cell::new(0));
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        seen.set(0);
        let error = execution::run(
            &team,
            |relay| {
                let poll = || relay(());
                q.try_mean_by_polled(
                    |row| execution::sum(values.iter().map(|v| v + row as f64), Some(&poll)),
                    Some(&poll),
                )
            },
            &mut |_| {
                assert_eq!(std::thread::current().id(), caller);
                seen.set(seen.get() + 1);
                if seen.get() == 3 {
                    Err(original.clone())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error, original);
        assert_eq!(error.return_code(), 430);
        assert_eq!(seen.get(), 3);
        let retried = q
            .try_mean_by_polled(
                |row| execution::sum(values.iter().map(|v| v + row as f64), None),
                None,
            )
            .unwrap();
        assert!((retried - 5123.75).abs() < 1e-10);
    }
}
