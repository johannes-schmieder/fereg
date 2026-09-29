use fereg_engine::{
    data::{DenseMatrix, PreparedWeights, ReferenceWeights},
    ppml::{
        control::*,
        family::Family,
        step::{Accepted, Point},
    },
    spec::{Estimator, Options},
    threads::ThreadTeam,
    ErrorCode,
};
fn point(finite: f64, absorbed: f64, n: usize) -> Point {
    Point {
        finite: vec![finite],
        absorbed: DenseMatrix::from_row_major(1, 1, vec![absorbed]).unwrap(),
        eta: vec![finite + absorbed; n],
    }
}
#[test]
fn rejected_full_steps_halve_predictors_and_both_coefficient_banks_together() {
    let q = ReferenceWeights::new(&PreparedWeights::None, 3, Estimator::Ppml).unwrap();
    let family = Family::new(&[1.; 3], q, None).unwrap();
    let mut baseline = None;
    for workers in [1, 2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let current = Accepted::start(&family, point(-2., 0., 3), &team).unwrap();
        let proposal = point(500., 500., 3);
        let next = current.try_step(&proposal, &team).unwrap();
        assert_eq!(next.halvings(), 9);
        assert_eq!(next.step(), 1. / 512.);
        assert_eq!(next.point().finite[0], -2. + 502. / 512.);
        assert_eq!(next.point().absorbed[(0, 0)], 500. / 512.);
        assert_eq!(
            next.point().eta,
            vec![next.point().finite[0] + next.point().absorbed[(0, 0)]; 3]
        );
        assert!(next.evaluation().objective.deviance < current.evaluation().objective.deviance);
        assert_eq!(current.point().eta, vec![-2.; 3]);
        assert_eq!(proposal.eta, vec![1000.; 3]);
        let result = (
            next.point().finite.clone(),
            next.point().eta.clone(),
            next.evaluation().objective,
        );
        if let Some(expected) = &baseline {
            assert_eq!(&result, expected);
        } else {
            baseline = Some(result);
        }
    }
}
#[test]
fn full_steps_preserve_proposal_bits_and_exhaustion_leaves_the_current_state_intact() {
    let q = ReferenceWeights::new(&PreparedWeights::None, 2, Estimator::Ppml).unwrap();
    let family = Family::new(&[1.; 2], q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let current = Accepted::start(&family, point(-2., 0., 2), &team).unwrap();
    let proposal = point(1e16, -1e16, 2);
    let optimum = current.try_step(&proposal, &team).unwrap();
    assert_eq!(optimum.step(), 1.);
    assert_eq!(optimum.halvings(), 0);
    assert_eq!(optimum.point(), &proposal);
    assert_eq!(
        optimum
            .try_step(&point(1000., 0., 2), &team)
            .err()
            .unwrap()
            .code,
        ErrorCode::NonConvergence
    );
    assert_eq!(optimum.point(), &proposal);
    assert_eq!(
        optimum
            .try_step(&point(0., 0., 3), &team)
            .err()
            .unwrap()
            .code,
        ErrorCode::InvalidArgument
    );
}
#[test]
fn finite_convex_parameters_survive_an_overflowing_endpoint_difference() {
    let q = ReferenceWeights::new(&PreparedWeights::None, 1, Estimator::Ppml).unwrap();
    let family = Family::new(&[1.], q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let current = Accepted::start(
        &family,
        Point {
            finite: vec![-1e308],
            absorbed: DenseMatrix::zeros(0, 1).unwrap(),
            eta: vec![-1.],
        },
        &team,
    )
    .unwrap();
    let candidate = Point {
        finite: vec![1e308],
        absorbed: DenseMatrix::zeros(0, 1).unwrap(),
        eta: vec![1.],
    };
    let next = current.try_step(&candidate, &team).unwrap();
    assert_eq!(next.step(), 0.5);
    assert_eq!(next.point().finite, vec![0.]);
    assert_eq!(next.point().eta, vec![0.]);
}
fn controls(budget: usize) -> Controls {
    Controls::new(&Options {
        iterations: budget,
        ..Options::for_estimator(Estimator::Ppml)
    })
    .unwrap()
}
const PASS: Scores = Scores {
    finite: 0.,
    fixed_effects: 0.,
};
#[test]
fn two_consecutive_score_and_objective_passes_request_strict_checks_without_resetting_budget() {
    let mut convergence = Convergence::new(controls(5));
    assert_eq!(
        convergence.observe(1., 1., PASS).unwrap(),
        Decision::Continue
    );
    assert_eq!(
        convergence.observe(1., 1., PASS).unwrap(),
        Decision::StrictChecksRequired
    );
    assert!(convergence.observe(1., 1., PASS).is_err());
    convergence.reject_strict().unwrap();
    assert_eq!(convergence.accepted_steps(), 2);
    assert_eq!(convergence.next_inner_tolerance(), 1e-12);
    assert_eq!(
        convergence.observe(1., 1., PASS).unwrap(),
        Decision::Continue
    );
    assert_eq!(
        convergence.observe(1., 1., PASS).unwrap(),
        Decision::StrictChecksRequired
    );
    convergence.reject_strict().unwrap();
    assert_eq!(
        convergence.observe(1., 1., PASS).unwrap_err().code,
        ErrorCode::NonConvergence
    );
    assert_eq!(convergence.accepted_steps(), 5);
}
#[test]
fn objective_stagnation_alone_cannot_converge_and_invalid_evidence_cannot_consume_a_step() {
    let mut convergence = Convergence::new(controls(3));
    assert!(convergence
        .observe(
            1.,
            1.,
            Scores {
                finite: f64::NAN,
                fixed_effects: 0.
            }
        )
        .is_err());
    assert_eq!(convergence.accepted_steps(), 0);
    assert_eq!(
        convergence
            .observe(
                1.,
                1.,
                Scores {
                    finite: 0.,
                    fixed_effects: 1e-3
                }
            )
            .unwrap(),
        Decision::Continue
    );
    assert_eq!(
        convergence.observe(1., 1., PASS).unwrap(),
        Decision::Continue
    );
    assert_eq!(
        convergence.observe(1., 1., PASS).unwrap(),
        Decision::StrictChecksRequired
    );
    assert_eq!(
        convergence.reject_strict().unwrap_err().code,
        ErrorCode::NonConvergence
    );
}
#[test]
fn inner_accuracy_tightens_monotonically_and_tiny_outer_tolerances_keep_a_strict_target() {
    let mut convergence = Convergence::new(controls(20));
    let mut previous = 1e-6;
    for iteration in 1..=10 {
        let target = convergence.next_inner_tolerance();
        assert!(target <= previous && target >= 1e-12);
        if iteration >= 8 {
            assert_eq!(target, 1e-12);
        }
        previous = target;
        convergence
            .observe(
                2.,
                1.,
                Scores {
                    finite: 0.5,
                    fixed_effects: 0.1,
                },
            )
            .unwrap();
    }
    let mut options = Options::for_estimator(Estimator::Ppml);
    options.tolerance = 1e-20;
    assert_eq!(Controls::new(&options).unwrap().strict_tolerance(), 0.1e-20);
    options.tolerance = f64::from_bits(1);
    assert!(Controls::new(&options).is_err());
}
