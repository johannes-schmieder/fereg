use super::*;
use crate::execution::interruption_tests::every_checkpoint;

#[test]
fn rectifier_metrics_and_normalized_rows_cancel_every_fixed_partition() {
    let n = 4099;
    let outcome: Vec<_> = (0..n)
        .map(|row| if row % 4 == 0 { 1. } else { 0. })
        .collect();
    let values: Vec<_> = (0..n)
        .map(|row| [1e-12, 2., -0.5, 0.125][row % 4])
        .collect();
    let fitted = DenseMatrix::from_columns(&[values]).unwrap();
    let expected = IrFixedPoint {
        direction_scale: 2.,
        interior_zero_relative: 5e-13,
        boundary_violation_relative: 0.25,
        boundary_maximum_relative: 1.,
    };
    assert_eq!(metrics(&outcome, &fitted, None).unwrap(), expected);
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let measured = team.install(|| every_checkpoint(|poll| metrics(&outcome, &fitted, poll)));
        let rectified = team.install(|| {
            every_checkpoint(|poll| {
                let mut target = vec![0.; n];
                rectify(&outcome, &fitted, 0.25, &mut target, poll)?;
                assert_eq!(
                    target,
                    (0..n)
                        .map(|row| if row % 4 == 1 { 1. } else { 0. })
                        .collect::<Vec<_>>()
                );
                Ok(target)
            })
        });
        assert!(measured > 10 && rectified > 5);
        assert_eq!(
            *reference.get_or_insert((measured, rectified)),
            (measured, rectified)
        );
    }
    let mut malformed = fitted;
    malformed[(1025, 0)] = f64::INFINITY;
    let observe = || true;
    assert_eq!(
        metrics(&outcome, &malformed, None).unwrap_err(),
        metrics(&outcome, &malformed, Some(&observe)).unwrap_err(),
    );
}
