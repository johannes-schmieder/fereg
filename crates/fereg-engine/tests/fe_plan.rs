#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
mod support;
use fereg_engine::{
    fe::*,
    spec::{Estimator, Options, Solver},
    ErrorCode,
};
#[test]
fn automatic_candidate_depends_on_structure_only() {
    for (levels, backend) in [
        (vec![], Backend::NoEffects),
        (vec![3], Backend::Direct),
        (vec![3, 2], Backend::SchurPcg),
    ] {
        for slopes in [false, true] {
            for repetitions in [2, 10] {
                for extra in [0, 50] {
                    let sample = support::prepared(&levels, slopes, repetitions, extra);
                    let weights = vec![2.; sample.n_active()];
                    for weights in [Weights::Unit, Weights::Provided(&weights)] {
                        for threads in [1, 8] {
                            let design = Design::new(&sample, weights).unwrap();
                            let options = Options {
                                threads,
                                ..Options::for_estimator(Estimator::Ols)
                            };
                            assert_eq!(
                                Plan::new(&design, Estimator::Ols, &options)
                                    .unwrap()
                                    .selected(),
                                backend
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn explicit_solver_support_and_degenerate_cmg_are_preserved() {
    for estimator in [Estimator::Ols, Estimator::Ppml] {
        for solver in [Solver::Lsmr, Solver::Cmg] {
            for effects in 0..=4 {
                for slopes in [false, true] {
                    let sample = support::prepared(&vec![3; effects], slopes, 2, 0);
                    let design = Design::new(&sample, Weights::Unit).unwrap();
                    let options = Options {
                        solver,
                        ..Options::for_estimator(estimator)
                    };
                    let expected = match (estimator, solver) {
                        (Estimator::Ols, Solver::Lsmr) => true,
                        (Estimator::Ols, Solver::Cmg) => !slopes || effects == 0,
                        (Estimator::Ppml, Solver::Lsmr) => effects >= 2 && !slopes,
                        (Estimator::Ppml, Solver::Cmg) => (2..=3).contains(&effects) && !slopes,
                        _ => unreachable!(),
                    };
                    let plan = Plan::new(&design, estimator, &options);
                    if expected {
                        let plan = plan.unwrap();
                        assert_eq!(plan.requested(), solver);
                        assert_eq!(
                            plan.selected(),
                            if solver == Solver::Lsmr {
                                Backend::Lsmr
                            } else {
                                Backend::Cmg
                            }
                        );
                        assert_eq!(
                            plan.fallback(),
                            if solver == Solver::Lsmr {
                                None
                            } else {
                                Some(Backend::Lsmr)
                            }
                        );
                    } else {
                        assert_eq!(plan.unwrap_err().code, ErrorCode::Unsupported);
                    }
                }
            }
        }
    }
}
#[test]
fn cmg_nuisance_bound_and_canonical_core_order_are_mathematical_support() {
    for (levels, supported) in [
        (vec![74, 72, 65], true),
        (vec![74, 72, 66], false),
        (vec![74, 72, 33, 33], true),
        (vec![74, 72, 34, 33], false),
    ] {
        let sample = support::prepared(&levels, false, 2, 0);
        let design = Design::new(&sample, Weights::Unit).unwrap();
        let options = Options {
            solver: Solver::Cmg,
            ..Options::for_estimator(Estimator::Ols)
        };
        let plan = Plan::new(&design, Estimator::Ols, &options);
        assert_eq!(plan.is_ok(), supported);
        if supported {
            assert_eq!(plan.unwrap().cmg_core(), Some([0, 1]));
        }
    }
    let sample = support::prepared(&[3, 5, 5], false, 2, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let plan = Plan::new(
        &design,
        Estimator::Ols,
        &Options::for_estimator(Estimator::Ols),
    )
    .unwrap();
    assert_eq!(plan.eliminated_effect(), Some(1));
    assert_eq!(plan.cmg_core(), Some([1, 2]));
}
#[test]
fn internal_overrides_validate_support_and_do_not_replace_explicit_requests() {
    let sample = support::prepared(&[3, 2], true, 2, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let options = Options::for_estimator(Estimator::Ppml);
    let plan = Plan::with_override(
        &design,
        Estimator::Ppml,
        &options,
        PlanOverride::Force(Backend::Lsmr),
    )
    .unwrap();
    assert_eq!(plan.selected(), Backend::Lsmr);
    assert_eq!(plan.requested(), Solver::Auto);
    assert!(Plan::with_override(
        &design,
        Estimator::Ols,
        &options,
        PlanOverride::Force(Backend::Direct)
    )
    .is_err());
    let explicit = Options {
        solver: Solver::Lsmr,
        ..options
    };
    assert!(Plan::with_override(
        &design,
        Estimator::Ols,
        &explicit,
        PlanOverride::Force(Backend::SchurPcg)
    )
    .is_err());
}
#[test]
fn linear_budgets_distinguish_ols_and_ppml_outer_iterations() {
    let sample = support::prepared(&[3, 2], false, 2, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let options = Options {
        iterations: 7,
        ..Options::for_estimator(Estimator::Ols)
    };
    assert_eq!(
        Plan::new(&design, Estimator::Ols, &options)
            .unwrap()
            .linear_budget(),
        7
    );
    assert_eq!(
        Plan::new(&design, Estimator::Ppml, &options)
            .unwrap()
            .linear_budget(),
        16_000
    );
}
