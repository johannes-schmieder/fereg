#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
mod support;
use fereg_engine::{
    data::DenseMatrix,
    fe::{Design, Weights},
    threads::ThreadTeam,
    ErrorCode,
};
fn explicit(design: &Design<'_>, sample: &fereg_engine::data::Prepared) -> Vec<Vec<f64>> {
    let mut matrix = vec![vec![0.; design.layout().total]; design.nobs()];
    for (effect, layout) in design.layout().effects.iter().enumerate() {
        let partition = design.topology().effect_partition(effect).unwrap();
        for (row, values) in matrix.iter_mut().enumerate() {
            for channel in 0..layout.channels {
                values[layout.offset + channel * layout.levels + partition.codes()[row] as usize] =
                    if design.topology().effects()[effect].intercept && channel == 0 {
                        1.0
                    } else {
                        let term = &design.topology().effects()[effect];
                        sample.regressors[term.slopes[channel - usize::from(term.intercept)]
                            .source
                            .get() as usize
                            - 1][row]
                    };
            }
        }
    }
    matrix
}
#[test]
fn operators_and_weighted_blocks_match_explicit_dummy_products() {
    let sample = support::prepared(&[3, 2], true, 4, 0);
    let weights: Vec<_> = (0..sample.n_active())
        .map(|row| (row % 5) as f64 * 0.5 + 0.5)
        .collect();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let matrix = explicit(&design, &sample);
    let coefficients = DenseMatrix::from_row_major(
        design.layout().total,
        3,
        (0..design.layout().total * 3)
            .map(|i| i as f64 * 0.25 - 2.)
            .collect(),
    )
    .unwrap();
    let rhs = DenseMatrix::from_row_major(
        design.nobs(),
        3,
        (0..design.nobs() * 3)
            .map(|i| i as f64 * 0.125 - 1.)
            .collect(),
    )
    .unwrap();
    let mut expected_multiply = DenseMatrix::zeros(design.nobs(), 3).unwrap();
    let mut expected_transpose = DenseMatrix::zeros(design.layout().total, 3).unwrap();
    for row in 0..design.nobs() {
        for column in 0..3 {
            for coefficient in 0..design.layout().total {
                expected_multiply[(row, column)] +=
                    matrix[row][coefficient] * coefficients[(coefficient, column)];
                expected_transpose[(coefficient, column)] +=
                    matrix[row][coefficient] * weights[row] * rhs[(row, column)];
            }
        }
    }
    for threads in [1, 2, 4, 8] {
        let team = ThreadTeam::new(threads).unwrap();
        assert_eq!(
            design.multiply(&coefficients, &team).unwrap(),
            expected_multiply
        );
        assert_eq!(
            design.transpose_weighted(&rhs, &team).unwrap(),
            expected_transpose
        );
        let residual = design.reconstruct(&rhs, &coefficients, &team).unwrap();
        for row in 0..design.nobs() {
            for column in 0..3 {
                assert_eq!(
                    residual[(row, column)],
                    rhs[(row, column)] - expected_multiply[(row, column)]
                );
            }
        }
    }
    for (effect, layout) in design.layout().effects.iter().enumerate() {
        for level in 0..layout.levels {
            let block = design.level_gram(effect, level).unwrap();
            for left in 0..layout.channels {
                for right in 0..layout.channels {
                    let a = design.coefficient_index(effect, left, level).unwrap();
                    let b = design.coefficient_index(effect, right, level).unwrap();
                    let expected: f64 = (0..design.nobs())
                        .map(|row| weights[row] * matrix[row][a] * matrix[row][b])
                        .sum();
                    assert_eq!(block[(left, right)], expected);
                }
            }
        }
    }
}
#[test]
fn empty_design_and_zero_rhs_preserve_shapes_without_fake_columns() {
    let sample = support::prepared(&[], false, 2, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let coefficients = DenseMatrix::zeros(0, 2).unwrap();
    let rhs = DenseMatrix::from_row_major(design.nobs(), 2, vec![3.; design.nobs() * 2]).unwrap();
    assert_eq!(
        design.multiply(&coefficients, &team).unwrap().as_slice(),
        vec![0.; design.nobs() * 2]
    );
    assert_eq!(design.reconstruct(&rhs, &coefficients, &team).unwrap(), rhs);
    assert_eq!(design.transpose_weighted(&rhs, &team).unwrap().nrows(), 0);
    let sample = support::prepared(&[3], true, 2, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let zero = DenseMatrix::zeros(design.layout().total, 0).unwrap();
    assert_eq!(
        design.multiply(&zero, &team).unwrap().nrows(),
        design.nobs()
    );
}
#[test]
fn malformed_inputs_and_nonrepresentable_products_fail_closed() {
    let sample = support::prepared(&[3], true, 2, 0);
    assert_eq!(
        Design::new(&sample, Weights::Provided(&[]))
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        Design::new(&sample, Weights::Provided(&[0.; 6]))
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    assert!(design.basis_value(99, 0, 0).is_err());
    assert!(design.coefficient_index(0, 9, 0).is_err());
    assert!(design
        .multiply(&DenseMatrix::zeros(1, 1).unwrap(), &team)
        .is_err());
    let invalid = DenseMatrix::from_row_major(
        design.layout().total,
        1,
        vec![f64::NAN; design.layout().total],
    )
    .unwrap();
    assert_eq!(
        design.multiply(&invalid, &team).unwrap_err().code,
        ErrorCode::InvalidData
    );
    let huge = DenseMatrix::from_row_major(
        design.layout().total,
        1,
        vec![f64::MAX; design.layout().total],
    )
    .unwrap();
    assert_eq!(
        design.multiply(&huge, &team).unwrap_err().code,
        ErrorCode::NonConvergence
    );
}
