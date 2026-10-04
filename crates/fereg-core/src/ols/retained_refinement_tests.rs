use super::*;
use crate::ols::prepared_scratch::PreparedOlsScratch;

struct Fixture {
    storage: RetainedWithinStorage,
    values: Vec<f64>,
    response: Vec<f64>,
    weights: Vec<f64>,
    bread: [f64; 4],
    provided: bool,
}

impl Fixture {
    fn new(scratch: bool, provided: bool) -> Self {
        let pattern = [(1., 2.), (1., 0.), (2., 3.), (2., 1.)];
        let errors = if provided {
            [2., -2., -1., 1.]
        } else {
            [1., -1., -1., 1.]
        };
        let mut values = Vec::new();
        let mut response = Vec::new();
        let mut weights = Vec::new();
        for i in 0..512 {
            let (u, v) = pattern[i % 4];
            values.extend([i as f64 + 7., u, -3. - i as f64, v]);
            response.push(2. * u - v / 2. + errors[i % 4] / 8.);
            weights.push(if i % 4 < 2 { 1. } else { 2. });
        }
        let storage = if scratch {
            let mut bank = PreparedOlsScratch::create_row_major(512, 4).unwrap();
            bank.write_linear(0, &values).unwrap();
            RetainedWithinStorage::Scratch(bank)
        } else {
            RetainedWithinStorage::Dense(
                crate::DenseMatrix::from_row_major(512, 4, values.clone()).unwrap(),
            )
        };
        // Invert the two-by-two weighted explicit-design Gram analytically.
        // Residual patterns are independently orthogonal to both raw columns.
        let bread = if provided {
            [1. / 16., -5. / 64., -5. / 64., 25. / 192.]
        } else {
            [63. / 2560., -15. / 512., -15. / 512., 25. / 512.]
        };
        Self {
            storage,
            values,
            response,
            weights,
            bread,
            provided,
        }
    }

    fn frame<'a>(&'a self, positions: &'a [usize], norms: &'a [f64]) -> RefinementFrame<'a> {
        RefinementFrame {
            storage: &self.storage,
            active_positions: positions,
            reference_norms: norms,
            response: &self.response,
            weights: if self.provided {
                BorrowedMapWeights::Provided(&self.weights)
            } else {
                BorrowedMapWeights::Unit
            },
            weight_scale: PreparedOlsBreadWeightScale {
                weight_kind: None,
                solver_weight_divisor: if self.provided { 4. } else { 1. },
                reference_weight_multiplier: if self.provided { 4. } else { 1. },
                reference_weight_sum: if self.provided { 768. } else { 512. },
            },
        }
    }

    fn check_storage(&self) {
        let mut reader = self.storage.reader();
        let mut row = [f64::NAN; 4];
        for i in 0..512 {
            reader.fill_row(i, &mut row).unwrap();
            assert!(row
                .iter()
                .zip(&self.values[i * 4..])
                .all(|(a, b)| a.to_bits() == b.to_bits()));
        }
    }
}

#[test]
fn noncontiguous_normalized_columns_refill_after_real_corrections() {
    for scratch in [false, true] {
        for provided in [false, true] {
            let fixture = Fixture::new(scratch, provided);
            let mut coefficients = [5.625, -1.875];
            let mut row = [f64::NAN; 4];
            let mut scores = [f64::NAN; 2];
            let mut corrections = [f64::NAN; 2];
            refine_retained_coefficients(
                fixture.frame(&[1, 3], &[1., 3., 1., 5.]),
                RefinementWorkspace {
                    coefficients: &mut coefficients,
                    bread: &fixture.bread,
                    row: &mut row,
                    scores: &mut scores,
                    corrections: &mut corrections,
                },
                1e-12,
            )
            .unwrap();
            assert!((coefficients[0] - 6.).abs() < 1e-13);
            assert!((coefficients[1] + 2.5).abs() < 1e-13);
            if let RetainedWithinStorage::Scratch(bank) = &fixture.storage {
                // Two full pages per pass; a real update requires a second pass.
                assert_eq!(bank.shared_read_chunks_for_test(), 4);
            }
            // Captured on the unchanged qualified parent before loop reuse.
            let (score_bits, correction_bits) = if provided {
                (
                    [4402127898266304512, 4398609461057421312],
                    [13600730137170542592, 0],
                )
            } else {
                (
                    [4407475922823806976, 4404520435568345088],
                    [4381861699943137280, 0],
                )
            };
            assert_eq!(
                coefficients.map(f64::to_bits),
                [4618441417868443648, 13836183955189006336]
            );
            assert_eq!(scores.map(f64::to_bits), score_bits);
            assert_eq!(corrections.map(f64::to_bits), correction_bits);
            fixture.check_storage();
        }
    }
}

#[test]
fn bounded_and_nonfinite_refinement_remain_fail_closed() {
    for scratch in [false, true] {
        for multiplier in [0.5, f64::INFINITY] {
            let fixture = Fixture::new(scratch, false);
            let bread = fixture.bread.map(|v| v * multiplier);
            let mut coefficients = [5.625, -1.875];
            let mut row = [0.; 4];
            let mut scores = [0.; 2];
            let mut corrections = [0.; 2];
            let error = refine_retained_coefficients(
                fixture.frame(&[1, 3], &[1., 3., 1., 5.]),
                RefinementWorkspace {
                    coefficients: &mut coefficients,
                    bread: &bread,
                    row: &mut row,
                    scores: &mut scores,
                    corrections: &mut corrections,
                },
                1e-12,
            )
            .unwrap_err();
            if multiplier.is_finite() {
                assert_eq!(error.code, ErrorCode::NonConvergence);
                assert_eq!(
                    error.message,
                    "retained coefficient refinement exhausted its accuracy budget"
                );
                assert!((coefficients[0] - 5.953125).abs() < 1e-13);
                assert!((coefficients[1] + 2.421875).abs() < 1e-13);
            } else {
                assert_eq!(error.code, ErrorCode::InvalidData);
                assert_eq!(
                    error.message,
                    "retained coefficient refinement is nonfinite"
                );
                assert_eq!(coefficients, [5.625, -1.875]);
            }
            fixture.check_storage();
        }
    }
}

#[test]
fn refinement_rejects_corrupt_omitted_scratch_column() {
    use std::io::{Seek, SeekFrom, Write};
    let fixture = Fixture::new(true, false);
    let RetainedWithinStorage::Scratch(bank) = &fixture.storage else {
        unreachable!()
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(bank.identity_path())
        .unwrap();
    file.seek(SeekFrom::Start(8 * 2)).unwrap();
    file.write_all(&f64::NAN.to_le_bytes()).unwrap();
    let mut coefficients = [5.625, -1.875];
    let mut row = [0.; 4];
    let mut scores = [0.; 2];
    let mut corrections = [0.; 2];
    let error = refine_retained_coefficients(
        fixture.frame(&[1, 3], &[1., 3., 1., 5.]),
        RefinementWorkspace {
            coefficients: &mut coefficients,
            bread: &fixture.bread,
            row: &mut row,
            scores: &mut scores,
            corrections: &mut corrections,
        },
        1e-12,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidData);
    assert_eq!(
        error.message,
        "prepared OLS scratch contains a nonfinite value"
    );
    assert_eq!(coefficients, [5.625, -1.875]);
}

#[test]
fn invalid_indices_reject_before_mutation_with_original_precedence() {
    let fixture = Fixture::new(false, false);
    for (positions, tolerance, message) in [
        (
            &[1, 1][..],
            1e-12,
            Some("retained coefficient refinement inputs are invalid"),
        ),
        (
            &[3, 1][..],
            1e-12,
            Some("retained coefficient refinement inputs are invalid"),
        ),
        (
            &[1, 1][..],
            f64::NAN,
            Some("retained coefficient accuracy tolerance is invalid"),
        ),
        (
            &[][..],
            f64::NAN,
            Some("retained coefficient accuracy tolerance is invalid"),
        ),
        (&[][..], 1e-12, None),
    ] {
        let mut coefficients = [5.625, -1.875];
        let mut row = [11., 12., 13., 14.];
        let mut scores = [21., 22.];
        let mut corrections = [31., 32.];
        let result = refine_retained_coefficients(
            fixture.frame(positions, &[1., 3., 1., 5.]),
            RefinementWorkspace {
                coefficients: &mut coefficients,
                bread: &fixture.bread,
                row: &mut row,
                scores: &mut scores,
                corrections: &mut corrections,
            },
            tolerance,
        );
        if let Some(message) = message {
            let error = result.unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidArgument);
            assert_eq!(error.message, message);
        } else {
            // Rank zero returns before incompatible workspace lengths are checked.
            result.unwrap();
        }
        assert_eq!(coefficients, [5.625, -1.875]);
        assert_eq!(row, [11., 12., 13., 14.]);
        assert_eq!(scores, [21., 22.]);
        assert_eq!(corrections, [31., 32.]);
    }
}
