use super::*;
use crate::{
    data::PreparedWeights, execution::interruption_tests::every_checkpoint, spec::Estimator,
};

#[test]
fn objective_blocks_initialization_and_fold_abort_without_changing_arithmetic() {
    let n = 4101;
    let outcome: Vec<_> = (0..n).map(|row| [0., 0.5, 2., 1., 4.][row % 5]).collect();
    let eta: Vec<_> = (0..n).map(|row| [-600., -8., 0., 7.][row % 4]).collect();
    for kind in [WeightKind::Frequency, WeightKind::Probability] {
        let weights = PreparedWeights::Provided {
            kind,
            values: (0..n).map(|row| (1 + row % 3) as f64).collect(),
        };
        let q = ReferenceWeights::new(&weights, n, Estimator::Ppml).unwrap();
        let family = Family::new(&outcome, q, None).unwrap();
        let mut reference = None;
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let checkpoints = every_checkpoint(|poll| family.evaluate_polled(&eta, &team, poll));
            assert!(checkpoints > 10);
            eprintln!("Poisson objective {kind:?}/{workers}: {checkpoints} exhaustive checkpoints");
            let evaluation = family.evaluate(&eta, &team).unwrap();
            assert!(evaluation.mean.iter().all(|mu| *mu > 0. && mu.is_finite()));
            assert_eq!(
                *reference.get_or_insert((evaluation, checkpoints)),
                (family.evaluate(&eta, &team).unwrap(), checkpoints)
            );
        }
    }
}

#[test]
fn objective_length_and_overflow_errors_keep_their_original_identity() {
    let weights = PreparedWeights::None;
    let q = ReferenceWeights::new(&weights, 2, Estimator::Ppml).unwrap();
    let family = Family::new(&[1., 0.], q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    for eta in [vec![0.], vec![0., 1000.]] {
        let expected = family.evaluate(&eta, &team).unwrap_err();
        assert_eq!(
            family
                .evaluate_polled(&eta, &team, Some(&|| true))
                .unwrap_err(),
            expected
        );
    }
}
