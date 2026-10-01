use super::*;
use crate::{
    data::{PreparedWeights, ReferenceWeights},
    execution::interruption_tests::every_checkpoint,
    spec::Estimator,
};

fn point(n: usize, eta: f64, reverse: bool) -> Point {
    let parameters: Vec<_> = (0..n)
        .map(|row| {
            if (row % 2 == 0) == reverse {
                1e308
            } else {
                -1e308
            }
        })
        .collect();
    Point {
        absorbed: DenseMatrix::from_row_major(n, 1, parameters.clone()).unwrap(),
        finite: parameters,
        eta: vec![eta; n],
    }
}
fn observed(accepted: Accepted<'_, '_>) -> (f64, usize, (Point, Evaluation)) {
    (accepted.step(), accepted.halvings(), accepted.into_parts())
}

#[test]
fn accepted_start_and_trial_interpolation_abort_every_checkpoint_and_restart_exactly() {
    let n = 4101;
    let outcome = vec![2.; n];
    let weights = PreparedWeights::None;
    let q = ReferenceWeights::new(&weights, n, Estimator::Ppml).unwrap();
    let family = Family::new(&outcome, q, None).unwrap();
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let start = every_checkpoint(|poll| {
            Accepted::start_polled(&family, point(n, 0., false), &team, poll).map(observed)
        });
        let accepted = Accepted::start(&family, point(n, 0., false), &team).unwrap();
        let initial = point(n, 0., false);
        let mut census = vec![start];
        for halvings in [false, true] {
            let candidate = point(n, 2_f64.ln() + if halvings { 3. } else { 0. }, true);
            let baseline = accepted.try_step(&candidate, &team).unwrap();
            assert_eq!(baseline.halvings() > 0, halvings);
            let count = every_checkpoint(|poll| {
                accepted
                    .try_step_polled(&candidate, &team, &mut |_| Ok(()), poll)
                    .map(observed)
            });
            assert!(count > 20);
            census.push(count);
            assert_eq!(
                observed(accepted.try_step(&candidate, &team).unwrap()),
                observed(baseline)
            );
            assert_eq!(accepted.point(), &initial);
            assert_eq!(accepted.evaluation().mean, vec![1.; n]);
        }
        eprintln!("Poisson point/start/trial {workers}: exhaustive census {census:?}");
        assert_eq!(*reference.get_or_insert(census.clone()), census);
    }
}

#[test]
fn line_search_host_r430_and_dense_abort_cannot_be_reclassified_as_numerical_stalls() {
    let weights = PreparedWeights::None;
    let q = ReferenceWeights::new(&weights, 2, Estimator::Ppml).unwrap();
    let family = Family::new(&[2., 2.], q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let accepted = Accepted::start(&family, point(2, 0., false), &team).unwrap();
    let candidate = point(2, 2_f64.ln(), true);
    let expected = FeregError::new(ErrorCode::NonConvergence, "line-search host r(430)");
    let Err(failure) =
        accepted.try_step_checked(&candidate, &team, &mut |_| Err(expected.clone()), None)
    else {
        panic!("host failure was accepted")
    };
    assert!(failure.callback);
    assert_eq!(failure.error, expected);
    let Err(failure) =
        accepted.try_step_checked(&candidate, &team, &mut |_| Ok(()), Some(&|| false))
    else {
        panic!("dense abort was accepted")
    };
    assert!(!failure.callback);
    assert_eq!(failure.error.code, ErrorCode::Interrupted);
    assert_eq!(accepted.point(), &point(2, 0., false));
}
