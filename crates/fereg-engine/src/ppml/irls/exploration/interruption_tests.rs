use super::*;
use crate::execution::interruption_tests::every_checkpoint;

#[test]
fn mu_thresholds_cancel_reference_rows_and_keep_binary_scaled_candidates() {
    let n = 4099;
    let outcome: Vec<_> = (0..n)
        .map(|row| if row % 3 == 0 { 1. } else { 0. })
        .collect();
    let mut mean: Vec<_> = (0..n)
        .map(|row| [2., 2_f64.powi(-29), 2_f64.powi(-27)][row % 3])
        .collect();
    mean[3072] = 2_f64.powi(-20);
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let count = team.install(|| {
            every_checkpoint(|poll| {
                let threshold = MeanThreshold::new_polled(&outcome, &mean, 2_f64.powi(-8), poll)?;
                assert_eq!(threshold.reference, 2_f64.powi(-20));
                assert!(!threshold.candidate(1., 0.));
                assert!(threshold.candidate(0., 2_f64.powi(-29)));
                assert!(threshold.candidate(0., 2_f64.powi(-28)));
                assert!(!threshold.candidate(0., 2_f64.powi(-27)));
                Ok(threshold.reference)
            })
        });
        assert!(count > 4);
        assert_eq!(*reference.get_or_insert(count), count);
    }
    for (outcome, mean) in [(&[][..], &[][..]), (&[0., 0.][..], &[1., 2.][..])] {
        assert!(
            every_checkpoint(|poll| Ok(
                MeanThreshold::new_polled(outcome, mean, 0.5, poll)?.reference
            )) > 0
        );
    }
}
