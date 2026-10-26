use super::{certify_frozen_combined_predictor, LinearReconstructionCertificate};

#[test]
fn joint_response_checks_match_separate_fresh_passes_and_reject_invalid_inputs() {
    use super::{check_joint_response, stable_weighted_norm, weighted_mean_slice};
    let n = 12_291;
    let mut reference: Vec<(u64, Option<u64>)> = Vec::new();
    for threads in [1, 4, 7] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut case = 0;
        for response_scale in [0., 1e-150, 1., 1e150] {
            for weight_scale in [1e-150, 1., 1e150] {
                let response: Vec<_> = (0..n)
                    .map(|i| ((i % 17) as f64 - 8.) * response_scale)
                    .collect();
                let residuals: Vec<_> = (0..n)
                    .map(|i| ((i % 13) as f64 - 6.) * response_scale)
                    .collect();
                let weights: Vec<_> = (0..n)
                    .map(|i| (0.5 + (i % 7) as f64) * weight_scale)
                    .collect();
                let expected = stable_weighted_norm(n, |i| Ok((response[i], weights[i])))
                    .unwrap()
                    .max(stable_weighted_norm(n, |i| Ok((residuals[i], weights[i]))).unwrap());
                let scale = super::super::PreparedOlsBreadWeightScale {
                    weight_kind: None,
                    solver_weight_divisor: 1.,
                    reference_weight_multiplier: 1.,
                    reference_weight_sum: n as f64,
                };
                for needs_mean in [false, true] {
                    let result = pool
                        .install(|| {
                            check_joint_response(&response, &residuals, &weights, needs_mean)
                        })
                        .unwrap();
                    // Fixed leaves reorder the sequential passes' rounding only;
                    // a leaf's direct square sum carries at most its length in
                    // relative rounding.
                    assert!((result.reference_norm - expected).abs() <= 1e-12 * expected);
                    assert_eq!(
                        result.reconstruction,
                        certify_frozen_combined_predictor(&response, &residuals).unwrap()
                    );
                    let mean = needs_mean.then(|| {
                        weighted_mean_slice(
                            &response,
                            crate::fe::BorrowedMapWeights::Provided(&weights),
                            scale,
                        )
                        .unwrap()
                    });
                    assert_eq!(result.response_mean.is_some(), mean.is_some());
                    if let (Some(actual), Some(mean)) = (result.response_mean, mean) {
                        let magnitude = 8. * response_scale;
                        assert!((actual - mean).abs() <= 1e-14 * magnitude.max(mean.abs()));
                    }
                    // The leaf tree, not the team, fixes every bit.
                    let bits = (
                        result.reference_norm.to_bits(),
                        result.response_mean.map(f64::to_bits),
                    );
                    if threads == 1 {
                        reference.push(bits);
                    } else {
                        assert_eq!(reference[case], bits);
                    }
                    case += 1;
                }
            }
        }
    }
    for bad in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(check_joint_response(&[1.], &[0.], &[bad], true).is_err());
    }
    assert!(check_joint_response(&[f64::NAN], &[0.], &[1.], false).is_err());
    assert!(check_joint_response(&[1.], &[f64::INFINITY], &[1.], false).is_err());
    assert!(check_joint_response(&[f64::MAX], &[0.], &[4.], true).is_err());
    assert!(check_joint_response(&[1.], &[], &[1.], false).is_err());
}

#[test]
fn reconstruction_matches_exact_rational_oracle_and_rejects_corruption() {
    let oracle = include_str!("../../../../../tests/fixtures/ppml/reconstruction.csv");
    let mut cancellation_rejections = 0;
    for line in oracle.lines().skip(1) {
        let fields: Vec<_> = line.split(',').collect();
        let values: Vec<_> = fields[1..6]
            .iter()
            .map(|hex| f64::from_bits(u64::from_str_radix(hex, 16).unwrap()))
            .collect();
        let [z, f, a, r, exact_discrepancy] = values[..] else {
            panic!("oracle shape")
        };
        let expected = fields[6] == "1";
        let mut certificate = LinearReconstructionCertificate::new();
        certificate
            .observe(z, f, a, r)
            .expect("finite oracle input");
        assert_eq!(
            certificate.passed, expected,
            "{}: {certificate:?}, exact discrepancy={exact_discrepancy}",
            fields[0]
        );
        if expected && certificate.relative_infinity_norm > 64.0 * f64::EPSILON {
            cancellation_rejections += 1;
            assert!(exact_discrepancy > 0.0);
        }
        if expected {
            // Both existing subtraction orders must obey the same bound;
            // neither is used to construct the independent oracle above.
            for computed in [(z - f) - r, (z - r) - f] {
                let mut constructed = LinearReconstructionCertificate::new();
                constructed.observe(z, f, computed, r).unwrap();
                assert!(constructed.passed, "{}: {constructed:?}", fields[0]);
            }
            assert!(
                certify_frozen_combined_predictor(&[z], &[r])
                    .unwrap()
                    .passed
            );
        }
    }
    assert!(
        cancellation_rejections >= 3,
        "must exercise the former cancellation failure"
    );
}

#[test]
fn reconstruction_rejects_nonfinite_and_overflowed_values() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for position in 0..4 {
            let mut values = [1.0, 0.25, 0.25, 0.5];
            values[position] = bad;
            let mut certificate = LinearReconstructionCertificate::new();
            assert!(certificate
                .observe(values[0], values[1], values[2], values[3])
                .is_err());
        }
    }
    let mut certificate = LinearReconstructionCertificate::new();
    assert!(certificate.observe(1.0, f64::MAX, f64::MAX, 0.0).is_err());
    assert!(certify_frozen_combined_predictor(&[f64::MAX], &[-f64::MAX]).is_err());
}

#[test]
fn reconstruction_failure_cannot_be_erased_by_a_later_row() {
    let mut certificate = LinearReconstructionCertificate::new();
    certificate.observe(1.0, 0.0, 1.001, 0.0).unwrap();
    assert!(!certificate.passed);
    let maximum = certificate.relative_infinity_norm;
    certificate.observe(1.0, 0.0, 1.0, 0.0).unwrap();
    assert!(!certificate.passed);
    assert_eq!(certificate.relative_infinity_norm, maximum);
}
