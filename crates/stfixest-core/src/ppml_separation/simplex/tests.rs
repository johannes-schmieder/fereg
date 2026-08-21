    use super::*;

    #[test]
    fn finds_two_regressor_certificate_from_primer() {
        let outcome = vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0];
        let x = DenseMatrix::from_columns(&[
            vec![2.0, -1.0, 0.0, 0.0, 5.0, 6.0],
            vec![-1.0, 2.0, 0.0, 0.0, -10.0, -12.0],
        ])
        .expect("matrix");
        let result = finite_simplex(&outcome, &x, None, 1e-10, 1e-8, 1000, 1_000_000)
            .expect("simplex");
        assert_eq!(result.status, MethodStatus::Certified);
        assert_eq!(result.dropped, vec![0]);
        assert!(result.certificate.expect("certificate").passed);
    }

    #[test]
    fn rejects_full_rank_positive_sample() {
        let outcome = vec![0.0, 1.0, 2.0];
        let x = DenseMatrix::from_columns(&[vec![1.0, 1.0, 1.0]]).expect("matrix");
        let result = finite_simplex(&outcome, &x, None, 1e-10, 1e-8, 100, 100_000)
            .expect("simplex");
        assert_eq!(result.status, MethodStatus::NoSeparation);
    }

    #[test]
    fn phase_one_combines_nullspace_directions() {
        // No single direction is nonnegative, but their sum is.
        let cone = vec![vec![2.0, -1.0], vec![-1.0, 2.0]];
        let (coordinates, _) = phase_one_cone(&cone, 1000, 1e-10)
            .expect("phase one")
            .expect("feasible");
        let values = cone
            .iter()
            .map(|row| row[0] * coordinates[0] + row[1] * coordinates[1])
            .collect::<Vec<_>>();
        assert!(values.iter().all(|value| *value >= -1e-8));
        assert!((values.iter().sum::<f64>() - 1.0).abs() < 1e-8);
    }
