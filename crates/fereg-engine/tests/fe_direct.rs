use fereg_engine::{
    data::*,
    fe::{Design, Weights},
    threads::ThreadTeam,
};
mod support;
use support::slope_sample as sample;
fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.),
        "{actual} != {expected}"
    );
}
#[test]
fn weighted_means_multiple_rhs_and_worker_identity() {
    let sample = sample(true, &[1, 0, 1, 0, 1], &[]);
    let weights = [1., 2., 3., 4., 2.];
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let rhs = DenseMatrix::from_columns(&[
        vec![1., 3., 7., 9., 5.],
        vec![2., 6., 14., 18., 10.],
        vec![0.; 5],
    ])
    .unwrap();
    let reference = design
        .direct_candidate(&rhs, &ThreadTeam::new(1).unwrap())
        .unwrap();
    assert_eq!(reference.level_ranks, [1, 1]);
    close(reference.coefficients[(0, 0)], 7., 1e-14);
    close(reference.coefficients[(1, 0)], 32. / 6., 1e-14);
    let moments = design
        .transpose_weighted(&reference.residuals, &ThreadTeam::new(1).unwrap())
        .unwrap();
    assert!(moments.as_slice().iter().all(|v| v.abs() < 1e-12));
    for threads in [2, 4, 8] {
        assert_eq!(
            reference,
            design
                .direct_candidate(&rhs, &ThreadTeam::new(threads).unwrap())
                .unwrap()
        );
    }
}
#[test]
fn rank_deficient_local_slopes_preserve_the_identified_span() {
    let codes = [0, 1, 0, 1, 0, 1, 0, 1];
    let slope = vec![-3., 2., -1., 2., 1., 2., 3., 2.];
    let duplicate = slope.iter().map(|v| 2. * v).collect();
    let sample = sample(
        true,
        &codes,
        &[slope.clone(), duplicate, vec![0.; 8], vec![1.; 8]],
    );
    let rhs = DenseMatrix::from_columns(&[slope.iter().map(|v| 3. + 5. * v).collect()]).unwrap();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let result = design
        .direct_candidate(&rhs, &ThreadTeam::new(4).unwrap())
        .unwrap();
    assert_eq!(result.level_ranks, [2, 1]);
    assert!(result.residuals.as_slice().iter().all(|v| v.abs() < 1e-13));
    // Canonical tied normalized channels win; nonidentified coordinates are zero.
    close(result.coefficients[(0, 0)], 3., 1e-13);
    close(result.coefficients[(2, 0)], 5., 1e-13);
    assert_eq!(result.coefficients[(4, 0)], 0.);
    assert_eq!(result.coefficients[(8, 0)], 0.);
}
#[test]
fn slope_only_zero_blocks_and_underdetermined_blocks_are_valid() {
    let sample = sample(
        false,
        &[0, 0, 1, 2],
        &[vec![0., 0., 2., 3.], vec![0., 0., 1., -1.]],
    );
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = DenseMatrix::from_columns(&[vec![3., 7., 4., 9.]]).unwrap();
    let result = design
        .direct_candidate(&rhs, &ThreadTeam::new(2).unwrap())
        .unwrap();
    assert_eq!(result.level_ranks, [0, 1, 1]);
    for (actual, expected) in result.residuals.as_slice().iter().zip([3., 7., 0., 0.]) {
        close(*actual, expected, 1e-14);
    }
}
#[test]
fn extreme_channel_and_common_weight_scales_leave_fits_unchanged() {
    let slope = vec![-3., -1., 1., 3.];
    for scale in [1e-300, 1., 1e300] {
        let sample = sample(false, &[0; 4], &[slope.iter().map(|x| x * scale).collect()]);
        for weight_scale in [1e-300, 1., 1e300] {
            let weights = vec![weight_scale; 4];
            let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
            let rhs = DenseMatrix::from_columns(&[slope.clone()]).unwrap();
            let result = design
                .direct_candidate(&rhs, &ThreadTeam::new(1).unwrap())
                .unwrap();
            assert_eq!(result.level_ranks, [1]);
            assert!(result.residuals.as_slice().iter().all(|v| v.abs() < 1e-14));
        }
    }
}
#[test]
fn near_collinear_slopes_keep_a_representable_direction() {
    let n = 40;
    let first: Vec<_> = (0..n).map(|r| r as f64 / 20. - 1.).collect();
    let signal: Vec<_> = (0..n).map(|r| if r % 2 == 0 { -1. } else { 1. }).collect();
    let second: Vec<_> = first
        .iter()
        .zip(&signal)
        .map(|(x, y)| x + 1e-7 * y)
        .collect();
    let sample = sample(true, &vec![0; n], &[first, second]);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let result = design
        .direct_candidate(
            &DenseMatrix::from_columns(&[signal]).unwrap(),
            &ThreadTeam::new(1).unwrap(),
        )
        .unwrap();
    assert_eq!(result.level_ranks, [3]);
    assert!(result.residuals.as_slice().iter().all(|v| v.abs() < 2e-8));
}
#[test]
fn empty_shapes_no_effects_and_invalid_calls() {
    let team = ThreadTeam::new(1).unwrap();
    let sample = support::prepared(&[], false, 2, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = DenseMatrix::from_columns(&[vec![2.; sample.n_active()]]).unwrap();
    let result = design.direct_candidate(&rhs, &team).unwrap();
    assert_eq!(result.residuals, rhs);
    assert_eq!(result.coefficients.nrows(), 0);
    let sample = support::prepared(&[3], true, 4, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let result = design
        .direct_candidate(&DenseMatrix::zeros(sample.n_active(), 0).unwrap(), &team)
        .unwrap();
    assert_eq!(result.residuals.ncols(), 0);
    assert!(design
        .direct_candidate(&DenseMatrix::zeros(1, 1).unwrap(), &team)
        .is_err());
    assert!(design
        .direct_candidate(
            &DenseMatrix::from_columns(&[vec![f64::NAN; sample.n_active()]]).unwrap(),
            &team
        )
        .is_err());
    let sample = support::prepared(&[2, 3], false, 3, 0);
    assert!(Design::new(&sample, Weights::Unit)
        .unwrap()
        .direct_candidate(&DenseMatrix::zeros(sample.n_active(), 1).unwrap(), &team)
        .is_err());
}
