use crate::metadata::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder,
    ExpandedTerm, LevelState, MetadataFlag, RawTermKind, SourceEntry, SourceId, TermComponent,
    TermId, TermOrder, TimeSeriesState,
};

use super::super::{
    prepare_model, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole, OlsRequestV1,
};
use super::*;

#[derive(Clone, Copy)]
enum TestWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [u32]),
}

#[test]
fn compensated_scaled_rows_preserve_scalar_bits_and_cancellation() {
    for width in [0, 1, 2, 3, 7, 8, 15, 16, 17, 31, 32, 63, 64, 65, 129] {
        let mut sums = vec![0.0; width];
        let mut corrections = vec![-0.0; width];
        let mut expected = sums.clone();
        let mut expected_corrections = corrections.clone();
        for row in 0..127 {
            let scale = [0.0, -0.0, 0.5, -1.0, 1e100, 1e-100][row % 6];
            let values: Vec<_> = (0..width)
                .map(|column| {
                    let magnitude = [0.0, 1e-150, 1e-16, 1.0, 1e16, 1e100][column % 6];
                    magnitude * ((row * 13 + column * 7) as f64).sin()
                })
                .collect();
            for column in 0..width {
                compensated_add(
                    scale * values[column],
                    &mut expected[column],
                    &mut expected_corrections[column],
                )
                .unwrap();
            }
            compensated_add_scaled_row(scale, &values, &mut sums, &mut corrections).unwrap();
            for column in 0..width {
                assert_eq!(sums[column].to_bits(), expected[column].to_bits());
                assert_eq!(
                    corrections[column].to_bits(),
                    expected_corrections[column].to_bits()
                );
            }
        }
    }
    let mut sums = [0.0; 3];
    let mut corrections = [0.0; 3];
    for value in [1e16, 1.0, -1e16] {
        compensated_add_scaled_row(value, &[1.0, -1.0, 0.5], &mut sums, &mut corrections).unwrap();
    }
    assert_eq!(
        sums.iter()
            .zip(corrections)
            .map(|(s, c)| s + c)
            .collect::<Vec<_>>(),
        [1.0, -1.0, 0.5]
    );
}

#[test]
fn compensated_scaled_rows_reject_shapes_and_nonfinite_coordinates() {
    let mut sums = [3.0, 4.0];
    let mut corrections = [0.25, 0.5];
    assert_eq!(
        compensated_add_scaled_row(1.0, &[2.0], &mut sums, &mut corrections)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        compensated_add_scaled_row(1.0, &[2.0, 3.0], &mut sums, &mut corrections[..1])
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(sums, [3.0, 4.0]);
    assert_eq!(corrections, [0.25, 0.5]);
    for (scale, value, sum, correction) in [
        (f64::NAN, 1.0, 0.0, 0.0),
        (1.0, f64::INFINITY, 0.0, 0.0),
        (f64::INFINITY, 0.0, 0.0, 0.0),
        (2.0, f64::MAX, 0.0, 0.0),
        (1.0, f64::MAX, f64::MAX, 0.0),
        (1.0, 1.0, f64::NAN, 0.0),
        (1.0, 1.0, 0.0, f64::INFINITY),
        // Half an ULP leaves this even sum unchanged but overflows the correction.
        (1.0, 2.0_f64.powi(970), 1e308, f64::MAX),
    ] {
        for position in [0, 1, 15, 31] {
            let mut values = [1.0; 32];
            let mut sums = [0.0; 32];
            let mut corrections = [0.0; 32];
            values[position] = value;
            sums[position] = sum;
            corrections[position] = correction;
            let mut scalar_sum = sum;
            let mut scalar_correction = correction;
            assert!(
                compensated_add(scale * value, &mut scalar_sum, &mut scalar_correction).is_err()
            );
            assert_eq!(
                compensated_add_scaled_row(scale, &values, &mut sums, &mut corrections)
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidData
            );
        }
    }
}

#[test]
fn unit_sum_of_squares_preserves_bits_and_errors() {
    for n in [0, 1, 127, 16_387] {
        let ones = vec![1.0; n];
        let unit = ReferenceWeightView {
            values: None,
            kind: None,
            divisor: 1.0,
            multiplier: 1.0,
        };
        let explicit = ReferenceWeightView {
            values: Some(&ones),
            kind: Some(WeightKind::Frequency),
            ..unit
        };
        for magnitude in [0.0, 1e-200, 1.0, 1e150, 1e300] {
            let mut values: Vec<_> = (0..n).map(|r| (r as f64).sin() * magnitude).collect();
            for center in [-0.0, 0.3 * magnitude, f64::NAN] {
                let a = weighted_sum_of_squares(&values, unit, center).map(f64::to_bits);
                let b = weighted_sum_of_squares(&values, explicit, center).map(f64::to_bits);
                assert_eq!(a, b);
            }
            if n > 0 {
                values[n / 2] = f64::INFINITY;
                assert_eq!(
                    weighted_sum_of_squares(&values, unit, 0.0),
                    weighted_sum_of_squares(&values, explicit, 0.0)
                );
            }
        }
    }
}

#[test]
fn unit_reference_mass_matches_explicit_weights_and_preserves_limits() {
    for n in [0, 1, 127, 16_384, 16_387] {
        let values = vec![1.0; n];
        let unit = ReferenceWeightView {
            values: None,
            kind: None,
            divisor: 1.0,
            multiplier: 1.0,
        };
        let explicit = ReferenceWeightView {
            values: Some(&values),
            kind: Some(WeightKind::Frequency),
            ..unit
        };
        assert_eq!(
            reference_weight_sum(unit, n),
            reference_weight_sum(explicit, n)
        );
        if n > 0 {
            let malformed = ReferenceWeightView {
                values: Some(&values),
                ..unit
            };
            assert_eq!(
                reference_weight_sum(malformed, n).unwrap_err(),
                malformed.value(0).unwrap_err()
            );
        }
    }
    let unit = ReferenceWeightView {
        values: None,
        kind: None,
        divisor: 1.0,
        multiplier: 1.0,
    };
    assert_eq!(
        reference_weight_sum(unit, 1_usize << 53).unwrap(),
        (1_u64 << 53) as f64
    );
    assert!(reference_weight_sum(unit, (1_usize << 53) + 1).is_err());
}

#[test]
fn outcome_stages_repeat_complete_results_and_fit_errors() {
    for n in [16_383, 16_384, 32_769] {
        let groups: Vec<_> = (0..n).map(|r| (r % 71) as f64).collect();
        let x: Vec<_> = (0..n).map(|r| ((r % 131) as f64 - 60.0) / 7.0).collect();
        let y: Vec<_> = (0..n)
            .map(|r| 1.7 * x[r] + groups[r] + 0.03 * (r as f64).sin())
            .collect();
        let raw = (0..n)
            .map(|r| 0.125 + (r % 17) as f64 / 11.0)
            .collect::<Vec<_>>();
        let frequency = (0..n).map(|r| 1 + (r % 7) as u32).collect::<Vec<_>>();
        for kind in [
            TestWeights::Unit,
            TestWeights::Analytic(&raw),
            TestWeights::Frequency(&frequency),
        ] {
            let mut prepared = prepared(&y, &[x.clone()], Some(&groups), true, kind);
            for threads in [1, 4, 8, 16] {
                let mut config = PreparedOlsUnadjustedConfig::default();
                config.fit.execution.threads =
                    crate::ThreadConfig::from_requested(threads, true).unwrap();
                let expected =
                    fit_prepared_model_unadjusted_with_loaded_moments(&prepared, config, None)
                        .unwrap();
                let actual =
                    fit_prepared_model_unadjusted_with_loaded_moments(&prepared, config, None)
                        .unwrap();
                assert_eq!(expected.absorbed_dof, actual.absorbed_dof);
                assert_eq!(expected.weights, actual.weights);
                assert_eq!(expected.dof, actual.dof);
                assert_eq!(expected.statistics, actual.statistics);
                assert_eq!(expected.covariance, actual.covariance);
                assert_eq!(expected.certificate, actual.certificate);
                let (a, b) = (expected.fit(), actual.fit());
                assert_eq!(a.coefficients(), b.coefficients());
                assert_eq!(a.rank_certificate(), b.rank_certificate());
                assert_eq!(a.score_certificate(), b.score_certificate());
                assert_eq!(a.bread(), b.bread());
                assert_eq!(a.execution(), b.execution());
                assert_eq!(a.residuals().len(), b.residuals().len());
                assert!(a
                    .residuals()
                    .iter()
                    .zip(b.residuals())
                    .all(|(a, b)| a.to_bits() == b.to_bits()));
                for (a, b) in [
                    (a.absorption_report(), b.absorption_report()),
                    (
                        a.residual_absorption_report(),
                        b.residual_absorption_report(),
                    ),
                ] {
                    let (mut a, mut b) = (a.clone(), b.clone());
                    for report in [&mut a, &mut b] {
                        report.setup_seconds = 0.0;
                        report.solve_seconds = 0.0;
                        report.certification_seconds = 0.0;
                    }
                    assert_eq!(a, b);
                }
            }
            prepared.sample.outcome[1] = f64::NAN;
            let mut config = PreparedOlsUnadjustedConfig::default();
            config.fit.execution.threads = crate::ThreadConfig::from_requested(16, true).unwrap();
            assert_eq!(
                fit_prepared_model_unadjusted_with_loaded_moments(&prepared, config, None)
                    .unwrap_err(),
                fit_prepared_model_unadjusted_with_loaded_moments(&prepared, config, None)
                    .unwrap_err()
            );
        }
    }
}

#[test]
fn multi_fe_outcome_stages_repeat_ordinary_and_categorical_inference() {
    let n = 16_385;
    let groups = [
        (0..n).map(|r| (r % 41) as f64).collect::<Vec<_>>(),
        (0..n).map(|r| ((r / 41) % 37) as f64).collect::<Vec<_>>(),
        (0..n).map(|r| ((r * 13) % 29) as f64).collect::<Vec<_>>(),
    ];
    let columns = (0..10)
        .map(|c| {
            (0..n)
                .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let y = (0..n)
        .map(|r| 1.7 * columns[0][r] + groups[0][r] + groups[1][r] + 0.03 * (r as f64).sin())
        .collect::<Vec<_>>();
    let raw = (0..n)
        .map(|r| 0.125 + (r % 17) as f64 / 11.0)
        .collect::<Vec<_>>();
    let frequency = (0..n).map(|r| 1 + (r % 7) as u32).collect::<Vec<_>>();
    for effects in [2, 3] {
        for kind in [
            TestWeights::Unit,
            TestWeights::Analytic(&raw),
            TestWeights::Frequency(&frequency),
        ] {
            let (mut request, mut input) =
                prepared_input(&y, &columns, Some(&groups[0]), true, kind);
            // Keep the legacy matrix indices and numerical traversal order explicit.
            #[allow(clippy::needless_range_loop)]
            for effect in 1..effects {
                let source = SourceId::new(request.model.sources.len() as u32 + 1).unwrap();
                request.model.sources.push(SourceEntry {
                    id: source,
                    canonical_name: format!("g{effect}"),
                });
                request.bindings.push(OlsInputBinding {
                    stata_var_position: request.bindings.len() as u32 + 1,
                    role: OlsInputRole::NumericSource(source),
                });
                request.absorbed_terms.push(AbsorbedTerm {
                    id: AbsorbTermId::new(effect as u32 + 1).unwrap(),
                    intercept: true,
                    categorical_sources: vec![source],
                    slope_sources: Vec::new(),
                    label: format!("g{effect}"),
                    saved_name: String::new(),
                });
                input
                    .numeric_sources
                    .push((source, loaded(&groups[effect])));
            }
            let ordinary = prepare_model(&request, &[], &input, false).unwrap();
            let categorical = crate::prepare_model(&request, &[], &input, true).unwrap();
            let mut config = PreparedOlsUnadjustedConfig::default();
            config.fit.execution.threads = crate::ThreadConfig::from_requested(8, true).unwrap();
            let expected =
                fit_prepared_model_unadjusted_with_loaded_moments(&ordinary, config, None).unwrap();
            let actual =
                fit_prepared_model_unadjusted_with_loaded_moments(&ordinary, config, None).unwrap();
            assert_eq!(expected.absorbed_dof, actual.absorbed_dof);
            assert_eq!(expected.dof, actual.dof);
            assert_eq!(expected.statistics, actual.statistics);
            assert_eq!(expected.covariance, actual.covariance);
            assert_eq!(expected.certificate, actual.certificate);
            assert_eq!(expected.fit.coefficients(), actual.fit.coefficients());
            assert_eq!(expected.fit.residuals(), actual.fit.residuals());
            let expected =
                fit_prepared_model_unadjusted_with_loaded_moments(&categorical, config, None)
                    .unwrap();
            let actual =
                fit_prepared_model_unadjusted_with_loaded_moments(&categorical, config, None)
                    .unwrap();
            assert_eq!(expected.absorbed_dof, actual.absorbed_dof);
            assert_eq!(expected.dof, actual.dof);
            assert_eq!(expected.statistics, actual.statistics);
            assert_eq!(expected.covariance, actual.covariance);
            assert_eq!(expected.certificate, actual.certificate);
            assert_eq!(expected.fit.coefficients(), actual.fit.coefficients());
            assert_eq!(expected.fit.residuals(), actual.fit.residuals());
        }
    }
}

#[test]
fn wide_outcome_stages_repeat_numeric_and_categorical_inference() {
    let n = 16_385;
    let groups: Vec<_> = (0..n).map(|r| (r % 71) as f64).collect();
    let mut columns: Vec<Vec<f64>> = (0..50)
        .map(|c| {
            (0..n)
                .map(|r| ((r * (c + 17) % 1009) as f64 / 11.0).sin())
                .collect()
        })
        .collect();
    columns[49] = columns[0].clone();
    let y: Vec<_> = (0..n)
        .map(|r| 1.7 * columns[0][r] + groups[r] + 0.03 * (r as f64).sin())
        .collect();
    let mut config = PreparedOlsUnadjustedConfig::default();
    config.fit.execution.threads = crate::ThreadConfig::from_requested(8, true).unwrap();
    let data = prepared(&y, &columns, Some(&groups), true, TestWeights::Unit);
    let expected = fit_prepared_model_unadjusted_with_loaded_moments(&data, config, None).unwrap();
    let actual = fit_prepared_model_unadjusted_with_loaded_moments(&data, config, None).unwrap();
    assert_eq!(expected.absorbed_dof, actual.absorbed_dof);
    assert_eq!(expected.weights, actual.weights);
    assert_eq!(expected.dof, actual.dof);
    assert_eq!(expected.statistics, actual.statistics);
    assert_eq!(expected.covariance, actual.covariance);
    assert_eq!(expected.certificate, actual.certificate);
    assert_eq!(expected.fit.coefficients(), actual.fit.coefficients());
    assert_eq!(expected.fit.bread(), actual.fit.bread());
    assert_eq!(
        expected.fit.score_certificate(),
        actual.fit.score_certificate()
    );
    assert!(expected
        .fit
        .residuals()
        .iter()
        .zip(actual.fit.residuals())
        .all(|(a, b)| a.to_bits() == b.to_bits()));

    let (mut request, input) = prepared_input(&y, &columns, Some(&groups), true, TestWeights::Unit);
    request.absorbed_terms[0].saved_name = "saved_g".to_owned();
    let data = crate::prepare_model(&request, &[], &input, true).unwrap();
    config.fit.allow_final_fe_recovery = true;
    config.fit.absorb.retain_coefficients = true;
    let expected = fit_prepared_model_unadjusted_with_loaded_moments(&data, config, None).unwrap();
    let actual = fit_prepared_model_unadjusted_with_loaded_moments(&data, config, None).unwrap();
    assert_eq!(expected.absorbed_dof, actual.absorbed_dof);
    assert_eq!(expected.weights, actual.weights);
    assert_eq!(expected.dof, actual.dof);
    assert_eq!(expected.statistics, actual.statistics);
    assert_eq!(expected.covariance, actual.covariance);
    assert_eq!(expected.certificate, actual.certificate);
    assert_eq!(expected.fit.coefficients(), actual.fit.coefficients());
    assert_eq!(expected.fit.bread(), actual.fit.bread());
    assert_eq!(
        expected.fit.score_certificate(),
        actual.fit.score_certificate()
    );
    assert!(expected
        .fit
        .residuals()
        .iter()
        .zip(actual.fit.residuals())
        .all(|(a, b)| a.to_bits() == b.to_bits()));
}

#[test]
fn categorical_outcome_stages_repeat_inference_and_fit_errors() {
    for n in [16_383, 16_385] {
        let groups: Vec<_> = (0..n).map(|r| (r % 71) as f64).collect();
        let x: Vec<_> = (0..n).map(|r| ((r % 131) as f64 - 60.0) / 7.0).collect();
        let y: Vec<_> = (0..n)
            .map(|r| 1.7 * x[r] + groups[r] + 0.03 * (r as f64).sin())
            .collect();
        let weights: Vec<_> = (0..n).map(|r| 1.0 + (r % 5) as f64).collect();
        for weight in [TestWeights::Unit, TestWeights::Analytic(&weights)] {
            let (mut request, input) =
                prepared_input(&y, &[x.clone(), x.clone()], Some(&groups), true, weight);
            request.absorbed_terms[0].saved_name = "saved_g".to_owned();
            let prepared = crate::prepare_model(&request, &[], &input, true).unwrap();
            for threads in [1, 8, 16] {
                let mut config = PreparedOlsUnadjustedConfig::default();
                config.fit.allow_final_fe_recovery = true;
                config.fit.absorb.retain_coefficients = true;
                config.fit.execution.threads =
                    crate::ThreadConfig::from_requested(threads, true).unwrap();
                let expected =
                    fit_prepared_model_unadjusted_with_loaded_moments(&prepared, config, None)
                        .unwrap();
                let actual =
                    fit_prepared_model_unadjusted_with_loaded_moments(&prepared, config, None)
                        .unwrap();
                assert_eq!(expected.absorbed_dof, actual.absorbed_dof);
                assert_eq!(expected.weights, actual.weights);
                assert_eq!(expected.dof, actual.dof);
                assert_eq!(expected.statistics, actual.statistics);
                assert_eq!(expected.covariance, actual.covariance);
                assert_eq!(expected.certificate, actual.certificate);
                let (a, b) = (expected.fit(), actual.fit());
                assert_eq!(a.coefficients(), b.coefficients());
                assert_eq!(a.rank_certificate(), b.rank_certificate());
                assert_eq!(a.score_certificate(), b.score_certificate());
                assert_eq!(a.bread(), b.bread());
                assert_eq!(a.execution(), b.execution());
                assert_eq!(a.residuals().len(), b.residuals().len());
                assert!(a
                    .residuals()
                    .iter()
                    .zip(b.residuals())
                    .all(|(a, b)| a.to_bits() == b.to_bits()));
                config.fit.score_tolerance = f64::NAN;
                assert_eq!(
                    fit_prepared_model_unadjusted_with_loaded_moments(&prepared, config, None)
                        .unwrap_err(),
                    fit_prepared_model_unadjusted_with_loaded_moments(&prepared, config, None)
                        .unwrap_err(),
                );
            }
        }
    }
}

#[test]
fn unit_mean_shortcut_matches_explicit_frequency_unit_mass_bits() {
    for n in [0, 1, 127, 16384, 16387] {
        let ones = vec![1.0; n];
        let unit = ReferenceWeightView {
            values: None,
            kind: None,
            divisor: 1.0,
            multiplier: 1.0,
        };
        let frequency = ReferenceWeightView {
            values: Some(&ones),
            kind: Some(WeightKind::Frequency),
            divisor: 1.0,
            multiplier: 1.0,
        };
        for scale in [1e-200, 1.0, 1e200] {
            for offset in [0.0, 1e6] {
                let mut values: Vec<_> = (0..n)
                    .map(|r| {
                        if r == 0 {
                            -0.0
                        } else {
                            (offset + (r % 101) as f64 - 50.0) * scale
                        }
                    })
                    .collect();
                let compare = |values: &[f64]| {
                    let a = weighted_mean(values, unit).map(f64::to_bits);
                    let b = weighted_mean(values, frequency).map(f64::to_bits);
                    assert_eq!(a, b);
                };
                compare(&values);
                if n > 0 {
                    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                        values[n / 2] = bad;
                        compare(&values);
                    }
                }
            }
        }
    }
}

#[test]
fn independent_parallel_fit_sums_preserve_bits_and_error_precedence() {
    for n in [16_383, 16_384, 32_769] {
        let mut outcome: Vec<_> = (0..n).map(|i| ((i % 97) as f64 - 43.0) * 1e90).collect();
        let mut residuals: Vec<_> = (0..n).map(|i| ((i % 41) as f64 - 17.0) * 1e-90).collect();
        let values: Vec<_> = (0..n).map(|i| 1.0 + (i % 7) as f64).collect();
        for kind in [
            None,
            Some(WeightKind::Frequency),
            Some(WeightKind::Analytic),
        ] {
            let weights = ReferenceWeightView {
                values: kind.map(|_| values.as_slice()),
                kind,
                divisor: 4.0,
                multiplier: 1.5,
            };
            for intercept in [false, true] {
                let expected =
                    independent_fit_sums(&outcome, &residuals, weights, intercept).unwrap();
                let actual =
                    crate::ThreadTeam::new(crate::ThreadConfig::from_requested(4, true).unwrap())
                        .install(|| independent_fit_sums(&outcome, &residuals, weights, intercept))
                        .unwrap()
                        .unwrap();
                assert_eq!(expected.0.to_bits(), actual.0.to_bits());
                assert_eq!(expected.1.to_bits(), actual.1.to_bits());
            }
        }
        outcome[1] = f64::NAN;
        residuals[0] = f64::INFINITY;
        let weights = ReferenceWeightView {
            values: None,
            kind: None,
            divisor: 1.0,
            multiplier: 1.0,
        };
        let error = crate::ThreadTeam::new(crate::ThreadConfig::from_requested(4, true).unwrap())
            .install(|| independent_fit_sums(&outcome, &residuals, weights, true))
            .unwrap()
            .unwrap_err();
        assert_eq!(
            error.message,
            "prepared iid weighted-mean value is not finite"
        );
    }
}

#[derive(Default)]
struct CountingReservation {
    calls: usize,
    fail_at: Option<usize>,
    undersize: bool,
}

impl InferenceReservation for CountingReservation {
    fn reserve<T>(
        &mut self,
        target: &mut Vec<T>,
        elements: usize,
        _label: &'static str,
    ) -> Result<()> {
        self.calls += 1;
        if self.fail_at == Some(self.calls) {
            return Err(allocation("injected prepared iid reservation failure"));
        }
        if !self.undersize {
            target
                .try_reserve_exact(elements)
                .map_err(|_| allocation("test prepared iid reservation failed"))?;
        }
        Ok(())
    }
}

#[test]
fn allocation_failures_and_undersized_reservations_fail_closed() {
    let prepared = prepared(
        &[1.2, 2.1, 2.9, 4.2, 5.1, 6.4],
        &[vec![-2.0, -1.0, 0.0, 1.0, 2.0, 4.0]],
        None,
        true,
        TestWeights::Unit,
    );
    let mut baseline = CountingReservation::default();
    let result = compose_for_test(&prepared, &mut baseline, InferencePerturbations::default())
        .expect("baseline prepared iid composition");
    assert!(result.certificate().passed);
    assert_eq!(baseline.calls, 5);

    for fail_at in 1..=baseline.calls {
        let mut reservation = CountingReservation {
            fail_at: Some(fail_at),
            ..CountingReservation::default()
        };
        let error = compose_for_test(
            &prepared,
            &mut reservation,
            InferencePerturbations::default(),
        )
        .expect_err("injected allocation must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    let mut undersized = CountingReservation {
        undersize: true,
        ..CountingReservation::default()
    };
    let error = compose_for_test(
        &prepared,
        &mut undersized,
        InferencePerturbations::default(),
    )
    .expect_err("undersized reservation must fail");
    assert_eq!(error.code, ErrorCode::Allocation);
}

#[test]
fn every_inference_perturbation_is_rejected() {
    let groups = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0];
    let x = [-2.0, 0.0, 1.0, -1.0, 2.0, 4.0, -3.0, 1.5, 5.0];
    let y = (0..x.len())
        .map(|row| 1.7 * x[row] + groups[row] + 0.07 * (row as f64).sin())
        .collect::<Vec<_>>();
    let prepared = prepared(&y, &[x.to_vec()], Some(&groups), true, TestWeights::Unit);
    for perturbation in [
        InferencePerturbations {
            rss: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            tss: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            residual_dof: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            variance_scale: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            covariance: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            standard_error: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            within_tss: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            r_squared: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            adjusted_r_squared: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            within_r_squared: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            adjusted_within_r_squared: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            root_mean_squared_error: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            f_statistic: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            gaussian_log_likelihood: true,
            ..InferencePerturbations::default()
        },
        InferencePerturbations {
            gaussian_null_log_likelihood: true,
            ..InferencePerturbations::default()
        },
    ] {
        let mut reservation = CountingReservation::default();
        let error = compose_for_test(&prepared, &mut reservation, perturbation)
            .expect_err("injected inference corruption must fail");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }
}

#[test]
fn hidden_constant_reported_constant_and_fe_only_edges_are_explicit() {
    let y = [2.0, 3.5, 1.0, 4.0, 5.0, 2.5];
    let no_intercept = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared(&y, &[], None, false, TestWeights::Unit),
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect("hidden structural-constant inference");
    assert_eq!(no_intercept.dof().absorbed_rank, 1);
    assert_eq!(no_intercept.dof().model_rank, 1);
    assert_eq!(no_intercept.dof().residual_dof, y.len() - 1);
    assert!(no_intercept.dof().has_structural_intercept);
    assert_eq!(
        no_intercept.absorbed_dof().effects[0].identity,
        crate::CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb
    );
    assert_eq!(no_intercept.covariance().dimension(), 0);
    let mean = y.iter().sum::<f64>() / y.len() as f64;
    assert_close(
        no_intercept.statistics().tss,
        y.iter().map(|value| (value - mean) * (value - mean)).sum(),
        1e-12,
    );
    assert!(no_intercept.statistics().f_statistic.is_none());

    let constant = [0.0; 6];
    let intercept_only = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared(&constant, &[], None, true, TestWeights::Unit),
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect("intercept-only exact inference");
    assert_eq!(intercept_only.dof().finite_rank, 0);
    assert_eq!(intercept_only.dof().finite_model_dof, 0);
    assert_eq!(intercept_only.dof().absorbed_rank, 1);
    assert_eq!(intercept_only.dof().model_rank, 1);
    assert_eq!(intercept_only.statistics().tss, 0.0);
    assert_eq!(intercept_only.statistics().rss, 0.0);
    assert!(intercept_only.statistics().r_squared.is_none());
    assert!(intercept_only.statistics().within_r_squared.is_none());
    assert!(intercept_only.statistics().f_statistic.is_none());
    assert!(intercept_only
        .statistics()
        .gaussian_log_likelihood
        .is_none());
    assert_eq!(intercept_only.covariance().values(), &[0.0]);

    let groups = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0];
    let explained = [1.0, 1.0, 3.0, 3.0, -2.0, -2.0];
    let fe_only = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared(&explained, &[], Some(&groups), false, TestWeights::Unit),
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect("FE-only rank-zero exact inference");
    assert_eq!(fe_only.dof().finite_rank, 0);
    assert_eq!(fe_only.dof().absorbed_rank, 3);
    assert_eq!(fe_only.covariance().dimension(), 0);
    assert!(fe_only.dof().has_structural_intercept);
    assert_eq!(fe_only.statistics().rss, 0.0);
    assert_eq!(fe_only.statistics().within_tss, 0.0);
    assert!(fe_only.statistics().f_statistic.is_none());
}

#[test]
fn pinned_constant_regressor_is_structurally_absorbed_and_cons_is_normalized() {
    // Pinned Stata 18/reghdfe contract: `c.one` followed by the default
    // `_cons` reports the normalized structural constant, while `one` is
    // classified as collinear with the virtual one-level absorbed effect.
    let y = [2.0, 5.0, 4.0, 8.0, 7.0, 11.0];
    let one = vec![1.0; y.len()];
    let result = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared(&y, &[one], None, true, TestWeights::Unit),
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect("constant plus requested intercept inference");
    assert_eq!(
        result.fit().coefficients()[0].status,
        PreparedOlsFitStatus::CollinearWithFixedEffects
    );
    assert_eq!(
        result.fit().coefficients()[1].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert!(result.fit().kept_generated_columns().is_empty());
    assert_eq!(result.fit().dropped_generated_columns(), &[0]);
    assert_eq!(result.dof().finite_rank, 0);
    assert_eq!(result.dof().finite_model_dof, 0);
    assert_eq!(result.dof().absorbed_rank, 1);
    assert!(result.dof().has_structural_intercept);
    assert!(result.statistics().f_statistic.is_none());
    let mean = y.iter().sum::<f64>() / y.len() as f64;
    let centered_tss = y
        .iter()
        .map(|value| (value - mean) * (value - mean))
        .sum::<f64>();
    assert_close(result.statistics().rss, centered_tss, 1e-12);
    assert_close(result.statistics().tss, centered_tss, 1e-12);
    assert_close(
        result
            .statistics()
            .gaussian_log_likelihood
            .expect("model ll"),
        result
            .statistics()
            .gaussian_null_log_likelihood
            .expect("null ll"),
        1e-12,
    );

    let hidden = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared(&y, &[vec![1.0; y.len()]], None, false, TestWeights::Unit),
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect("hidden constant-regressor inference");
    assert_eq!(
        hidden.fit().coefficients()[0].status,
        PreparedOlsFitStatus::CollinearWithFixedEffects
    );
    assert_eq!(hidden.dof().finite_rank, 0);
    assert_eq!(hidden.dof().finite_model_dof, 0);
    assert_eq!(hidden.dof().absorbed_rank, 1);
    assert!(!hidden.dof().has_normalized_absorbed_intercept);
    assert_eq!(hidden.covariance().dimension(), 0);
    assert_slice_close(result.fit().residuals(), hidden.fit().residuals(), 1e-12);
    for (reported, unreported) in [
        (result.statistics().rss, hidden.statistics().rss),
        (result.statistics().tss, hidden.statistics().tss),
        (
            result.statistics().within_tss,
            hidden.statistics().within_tss,
        ),
        (
            result.statistics().root_mean_squared_error,
            hidden.statistics().root_mean_squared_error,
        ),
    ] {
        assert_close(reported, unreported, 1e-12);
    }
}

#[test]
fn paired_no_x_default_and_noconstant_share_weighted_structural_residuals() {
    let y = [2.0, 7.0, 3.5, 11.0, 5.0, 13.0];
    let analytic = [0.5, 2.0, 1.5, 3.0, 0.75, 4.0];
    let groups = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0];
    for fixed_effect in [None, Some(groups.as_slice())] {
        let reported = fit_prepared_model_unadjusted_with_loaded_moments(
            &prepared(
                &y,
                &[],
                fixed_effect,
                true,
                TestWeights::Analytic(&analytic),
            ),
            PreparedOlsUnadjustedConfig::default(),
            None,
        )
        .expect("reported no-X inference");
        let hidden = fit_prepared_model_unadjusted_with_loaded_moments(
            &prepared(
                &y,
                &[],
                fixed_effect,
                false,
                TestWeights::Analytic(&analytic),
            ),
            PreparedOlsUnadjustedConfig::default(),
            None,
        )
        .expect("hidden no-X inference");
        let weighted_mean = y
            .iter()
            .zip(analytic)
            .map(|(value, weight)| value * weight)
            .sum::<f64>()
            / analytic.iter().sum::<f64>();
        assert_close(
            reported.fit().coefficients()[0]
                .value
                .expect("normalized reported mean"),
            weighted_mean,
            1e-12,
        );
        assert_eq!(
            reported.fit().coefficients()[0].status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert!(hidden.fit().coefficients().is_empty());
        assert_eq!(reported.dof().finite_rank, 0);
        assert_eq!(hidden.dof().finite_rank, 0);
        assert_eq!(reported.dof().finite_model_dof, 0);
        assert_eq!(hidden.dof().finite_model_dof, 0);
        assert_eq!(reported.dof().model_rank, hidden.dof().model_rank);
        assert_eq!(reported.dof().residual_dof, hidden.dof().residual_dof);
        assert_eq!(reported.covariance().dimension(), 1);
        assert_eq!(hidden.covariance().dimension(), 0);
        assert_slice_close(reported.fit().residuals(), hidden.fit().residuals(), 1e-12);
        for (left, right) in [
            (reported.statistics().rss, hidden.statistics().rss),
            (reported.statistics().tss, hidden.statistics().tss),
            (
                reported.statistics().within_tss,
                hidden.statistics().within_tss,
            ),
            (
                reported.statistics().root_mean_squared_error,
                hidden.statistics().root_mean_squared_error,
            ),
            (
                reported
                    .statistics()
                    .gaussian_log_likelihood
                    .expect("reported likelihood"),
                hidden
                    .statistics()
                    .gaussian_log_likelihood
                    .expect("hidden likelihood"),
            ),
        ] {
            assert_close(left, right, 1e-12);
        }
        assert!(reported.statistics().f_statistic.is_none());
        assert!(hidden.statistics().f_statistic.is_none());
    }
}

#[test]
fn pinned_full_ibn_span_has_one_finite_contrast_and_report_only_cons() {
    let foreign = SourceEntry {
        id: SourceId::new(1).expect("foreign source"),
        canonical_name: "foreign".to_owned(),
    };
    let factor_term = |id: u32, level: f64| ExpandedTerm {
        id: TermId::new(id).expect("factor term"),
        order: TermOrder::new(id).expect("factor order"),
        coefficient_slot: CoefficientSlot::new(id).expect("factor slot"),
        raw_kind: RawTermKind::Factor,
        whole_column_omitted: false,
        canonical_token: format!("{level}.foreign"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("factor component"),
            source: foreign.id,
            kind: ComponentKind::Categorical,
            level_state: LevelState::Present,
            level_value: level,
            base: MetadataFlag::False,
            omitted: MetadataFlag::False,
            raw_operator: level.to_string(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    };
    let foreign_values = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let y = [9.5, 10.0, 10.5, 12.5, 13.0, 13.5];
    let fit = |include_intercept| {
        let prepared = prepare_model(
            &OlsRequestV1 {
                model: CompiledTermGraph {
                    sources: vec![foreign.clone()],
                    terms: vec![factor_term(1, 0.0), factor_term(2, 1.0)],
                },
                absorbed_terms: Vec::new(),
                bindings: vec![
                    OlsInputBinding {
                        stata_var_position: 1,
                        role: OlsInputRole::Candidate,
                    },
                    OlsInputBinding {
                        stata_var_position: 2,
                        role: OlsInputRole::Outcome,
                    },
                    OlsInputBinding {
                        stata_var_position: 3,
                        role: OlsInputRole::NumericSource(foreign.id),
                    },
                ],
                include_intercept,
                keep_singletons: true,
            },
            &[],
            &LoadedOlsInput {
                candidate: vec![true; y.len()].into(),
                outcome: loaded(&y),
                numeric_sources: vec![(foreign.id, loaded(&foreign_values))],
                analytic_weight: None,
                frequency_weight: None,
                probability_weight: None,
            },
            false,
        )
        .expect("full ibn preparation");
        fit_prepared_model_unadjusted_with_loaded_moments(
            &prepared,
            PreparedOlsUnadjustedConfig::default(),
            None,
        )
        .expect("full ibn inference")
    };
    let reported = fit(true);
    let hidden = fit(false);
    for result in [&reported, &hidden] {
        assert_eq!(result.dof().finite_rank, 1);
        assert_eq!(result.dof().finite_model_dof, 1);
        assert_eq!(result.dof().absorbed_rank, 1);
        assert_eq!(result.dof().model_rank, 2);
        assert_eq!(result.fit().kept_generated_columns(), &[0]);
        assert_eq!(result.fit().dropped_generated_columns(), &[1]);
        assert!(result.statistics().f_statistic.is_some());
    }
    assert_eq!(
        reported.fit().coefficients()[2].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_eq!(reported.covariance().dimension(), 2);
    assert_eq!(hidden.fit().coefficients().len(), 2);
    assert_eq!(hidden.covariance().dimension(), 1);
}

#[test]
fn pinned_default_and_noconstant_pairs_share_the_structural_fit_for_unit_a_and_f_weights() {
    let groups = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    let x = vec![-2.0, -0.5, 1.0, 3.0, -3.0, -1.0, 2.0, 4.0];
    let noise = [0.2, -0.1, 0.3, -0.2, -0.3, 0.1, -0.2, 0.2];
    let y = x
        .iter()
        .enumerate()
        .map(|(row, x)| 4.0 + 1.8 * x + 2.5 * groups[row] + noise[row])
        .collect::<Vec<_>>();
    let analytic = [0.5, 1.5, 2.0, 0.8, 1.2, 2.5, 0.7, 1.8];
    let frequency = [1_u32, 2, 3, 1, 2, 4, 1, 2];

    for weights in [
        TestWeights::Unit,
        TestWeights::Analytic(&analytic),
        TestWeights::Frequency(&frequency),
    ] {
        for fixed_effect in [None, Some(groups.as_slice())] {
            let reported = fit_prepared_model_unadjusted_with_loaded_moments(
                &prepared(&y, std::slice::from_ref(&x), fixed_effect, true, weights),
                PreparedOlsUnadjustedConfig::default(),
                None,
            )
            .expect("reported structural-constant inference");
            let hidden = fit_prepared_model_unadjusted_with_loaded_moments(
                &prepared(&y, std::slice::from_ref(&x), fixed_effect, false, weights),
                PreparedOlsUnadjustedConfig::default(),
                None,
            )
            .expect("hidden structural-constant inference");

            assert_eq!(reported.dof().finite_rank, hidden.dof().finite_rank);
            assert_eq!(
                reported.dof().finite_model_dof,
                hidden.dof().finite_model_dof
            );
            assert_eq!(reported.dof().absorbed_rank, hidden.dof().absorbed_rank);
            assert_eq!(reported.dof().model_rank, hidden.dof().model_rank);
            assert_eq!(reported.dof().residual_dof, hidden.dof().residual_dof);
            assert!(reported.dof().has_normalized_absorbed_intercept);
            assert!(!hidden.dof().has_normalized_absorbed_intercept);
            assert_eq!(reported.absorbed_dof(), hidden.absorbed_dof());
            assert!(reported.dof().has_structural_intercept);
            assert_eq!(reported.dof().finite_model_dof, 1);
            assert_eq!(reported.fit().coefficients().len(), 2);
            assert_eq!(hidden.fit().coefficients().len(), 1);
            assert_eq!(
                reported.fit().coefficients()[1].status,
                PreparedOlsFitStatus::NormalizedAbsorbedIntercept
            );
            assert_close(
                reported.fit().coefficients()[0]
                    .value
                    .expect("reported slope"),
                hidden.fit().coefficients()[0].value.expect("hidden slope"),
                1e-11,
            );
            assert_eq!(
                reported.fit().residuals().len(),
                hidden.fit().residuals().len()
            );
            for (left, right) in reported
                .fit()
                .residuals()
                .iter()
                .zip(hidden.fit().residuals())
            {
                assert_close(*left, *right, 1e-11);
            }
            for (left, right) in [
                (reported.statistics().rss, hidden.statistics().rss),
                (reported.statistics().tss, hidden.statistics().tss),
                (
                    reported.statistics().within_tss,
                    hidden.statistics().within_tss,
                ),
                (
                    reported.statistics().root_mean_squared_error,
                    hidden.statistics().root_mean_squared_error,
                ),
                (
                    reported.statistics().f_statistic.expect("reported F"),
                    hidden.statistics().f_statistic.expect("hidden F"),
                ),
            ] {
                assert_close(left, right, 1e-10);
            }
            assert_eq!(reported.covariance().dimension(), 2);
            assert_eq!(hidden.covariance().dimension(), 1);
            assert_close(
                reported.covariance().values()[0],
                hidden.covariance().values()[0],
                1e-10,
            );
            if fixed_effect.is_none() {
                assert_eq!(
                    reported.absorbed_dof().effects[0].identity,
                    crate::CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb
                );
            } else {
                assert!(matches!(
                    reported.absorbed_dof().effects[0].identity,
                    crate::CategoricalTopologyDofEffectIdentity::Explicit { .. }
                ));
            }
        }
    }
}

#[test]
fn wald_f_survives_weak_signal_under_large_orthogonal_noise() {
    let x = vec![-3.5, -2.5, -1.5, -0.5, 0.5, 1.5, 2.5, 3.5];
    let orthogonal_noise = [1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0];
    let y = x
        .iter()
        .zip(orthogonal_noise)
        .map(|(x, noise)| 1.0e12 * noise + 10.0 * x)
        .collect::<Vec<_>>();
    let result = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared(&y, &[x], None, true, TestWeights::Unit),
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect("weak-signal Wald inference");
    let f_statistic = result
        .statistics()
        .f_statistic
        .expect("positive-variance slope test");
    assert!(f_statistic > 0.0 && f_statistic < 1.0e-18);
    assert!(result.certificate().passed);
    assert!(
        result
            .certificate()
            .statistic_formula_relative_infinity_norm
            < 5.0e-10
    );
}

#[test]
fn invalid_configs_pweights_and_nonpositive_residual_dof_fail_closed() {
    let y = [1.0, 2.0, 3.0, 4.0];
    let x = vec![vec![0.0, 1.0, 2.0, 3.0]];
    for tolerance in [f64::NAN, 0.0, 1.0] {
        let error = fit_prepared_model_unadjusted_with_loaded_moments(
            &prepared(&y, &x, None, true, TestWeights::Unit),
            PreparedOlsUnadjustedConfig {
                certificate_tolerance: tolerance,
                ..PreparedOlsUnadjustedConfig::default()
            },
            None,
        )
        .expect_err("invalid inference tolerance must fail");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    let pweights = [0.5, 1.5, 2.0, 3.0];
    let error = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared(&y, &x, None, true, TestWeights::Probability(&pweights)),
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect_err("iid pweight inference must fail");
    assert_eq!(error.code, ErrorCode::Unsupported);

    let saturated = prepared(&[2.0], &[], None, true, TestWeights::Unit);
    let error = fit_prepared_model_unadjusted_with_loaded_moments(
        &saturated,
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect_err("zero residual DoF must fail");
    assert_eq!(error.code, ErrorCode::InsufficientObservations);

    let empty = prepared(&[], &[], None, false, TestWeights::Unit);
    let error = fit_prepared_model_unadjusted_with_loaded_moments(
        &empty,
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect_err("empty inference sample must fail");
    assert_eq!(error.code, ErrorCode::InsufficientObservations);

    let mut misaligned = prepared(&y, &x, None, true, TestWeights::Analytic(&pweights));
    let PreparedWeights::Provided { values, .. } = &mut misaligned.sample.weights else {
        panic!("analytic test weights must be present");
    };
    values.pop();
    let error = fit_prepared_model_unadjusted_with_loaded_moments(
        &misaligned,
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect_err("misaligned prepared weights must fail");
    assert_eq!(error.code, ErrorCode::InvalidData);
}

#[test]
fn corrupted_absorbed_dof_artifact_cannot_be_composed() {
    let groups = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0];
    let with_fe = prepared(
        &[1.0, 1.2, 2.5, 2.8, 4.1, 4.4],
        &[vec![-2.0, 0.5, -1.0, 2.0, 1.5, 4.0]],
        Some(&groups),
        true,
        TestWeights::Unit,
    );
    let fit = fit_prepared_model_coefficients(&with_fe, PreparedOlsFitConfig::default())
        .expect("certified fit");
    let mut absorbed = compute_canonical_prepared_ols_absorbed_dof(
        &with_fe,
        PreparedOlsAbsorbedDofPolicy::Pairwise,
    )
    .expect("certified absorbed DoF");
    absorbed.effects[0].categories += 1;
    absorbed.initial_categories += 1;
    absorbed.absorbed_rank += 1;
    let mut reservation = CountingReservation::default();
    let error = compose_internal(
        &with_fe,
        fit,
        absorbed,
        PreparedOlsUnadjustedConfig::default().certificate_tolerance,
        &mut reservation,
        InferencePerturbations::default(),
    )
    .expect_err("corrupted absorbed-DoF report must fail");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(reservation.calls, 0);

    let no_absorb = prepared(
        &[1.0, 2.0, 4.0, 7.0],
        &[vec![-1.0, 0.0, 1.0, 3.0]],
        None,
        true,
        TestWeights::Unit,
    );
    let fit = fit_prepared_model_coefficients(&no_absorb, PreparedOlsFitConfig::default())
        .expect("no-absorb fit");
    let mut absorbed = compute_canonical_prepared_ols_absorbed_dof(
        &no_absorb,
        PreparedOlsAbsorbedDofPolicy::Pairwise,
    )
    .expect("implicit no-absorb DoF");
    absorbed.effects[0].identity = crate::CategoricalTopologyDofEffectIdentity::Explicit {
        term_id: AbsorbTermId::new(1).expect("fake term"),
        sources: vec![SourceId::new(1).expect("fake source")],
    };
    let mut reservation = CountingReservation::default();
    let error = compose_internal(
        &no_absorb,
        fit,
        absorbed,
        PreparedOlsUnadjustedConfig::default().certificate_tolerance,
        &mut reservation,
        InferencePerturbations::default(),
    )
    .expect_err("implicit DoF identity corruption must fail");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(reservation.calls, 0);
}

#[test]
fn analytic_and_frequency_reference_scales_are_reported_exactly() {
    let y = [1.1, 2.3, 3.0, 4.4, 5.2, 6.1];
    let x = vec![vec![-2.0, -1.0, 0.0, 1.0, 2.0, 4.0]];
    let analytic = [0.25, 2.0, 1.5, 3.0, 0.75, 4.0];
    let aresult = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared(&y, &x, None, true, TestWeights::Analytic(&analytic)),
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect("analytic iid inference");
    assert_eq!(aresult.weights().effective_observations, 6);
    assert_close(
        aresult.weights().raw_weight_sum.expect("raw analytic sum"),
        analytic.iter().sum(),
        1e-12,
    );

    let frequencies = [1_u32, 2, 3, 1, 4, 2];
    let fresult = fit_prepared_model_unadjusted_with_loaded_moments(
        &prepared(&y, &x, None, true, TestWeights::Frequency(&frequencies)),
        PreparedOlsUnadjustedConfig::default(),
        None,
    )
    .expect("frequency iid inference");
    assert_eq!(fresult.weights().physical_observations, 6);
    assert_eq!(fresult.weights().effective_observations, 13);
    assert_eq!(fresult.weights().raw_weight_sum, Some(13.0));
}

fn compose_for_test<R: InferenceReservation>(
    prepared: &PreparedOlsData,
    reservation: &mut R,
    perturbations: InferencePerturbations,
) -> Result<CertifiedPreparedOlsUnadjusted> {
    let fit = fit_prepared_model_coefficients(prepared, PreparedOlsFitConfig::default())?;
    let absorbed = compute_canonical_prepared_ols_absorbed_dof(
        prepared,
        PreparedOlsAbsorbedDofPolicy::Pairwise,
    )?;
    compose_internal(
        prepared,
        fit,
        absorbed,
        PreparedOlsUnadjustedConfig::default().certificate_tolerance,
        reservation,
        perturbations,
    )
}

fn prepared(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    fixed_effect: Option<&[f64]>,
    include_intercept: bool,
    weights: TestWeights<'_>,
) -> PreparedOlsData {
    let (request, input) = prepared_input(
        outcome,
        regressors,
        fixed_effect,
        include_intercept,
        weights,
    );
    prepare_model(&request, &[], &input, false).expect("prepared iid test case")
}

fn prepared_input(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    fixed_effect: Option<&[f64]>,
    include_intercept: bool,
    weights: TestWeights<'_>,
) -> (OlsRequestV1, LoadedOlsInput) {
    assert!(regressors
        .iter()
        .all(|column| column.len() == outcome.len()));
    let mut sources = Vec::new();
    let mut terms = Vec::new();
    let mut numeric_sources = Vec::new();
    let mut bindings = vec![
        OlsInputBinding {
            stata_var_position: 1,
            role: OlsInputRole::Candidate,
        },
        OlsInputBinding {
            stata_var_position: 2,
            role: OlsInputRole::Outcome,
        },
    ];
    for (index, values) in regressors.iter().enumerate() {
        let one_based = u32::try_from(index + 1).expect("small source");
        let source = SourceEntry {
            id: SourceId::new(one_based).expect("source"),
            canonical_name: format!("x{one_based}"),
        };
        terms.push(ExpandedTerm {
            id: TermId::new(one_based).expect("term"),
            order: TermOrder::new(one_based).expect("term order"),
            coefficient_slot: CoefficientSlot::new(one_based).expect("coefficient slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: format!("c.x{one_based}"),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source: source.id,
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        });
        bindings.push(OlsInputBinding {
            stata_var_position: one_based + 2,
            role: OlsInputRole::NumericSource(source.id),
        });
        numeric_sources.push((source.id, loaded(values)));
        sources.push(source);
    }
    let absorbed_terms = fixed_effect.map_or_else(Vec::new, |codes| {
        let one_based = u32::try_from(sources.len() + 1).expect("small FE source");
        let source = SourceEntry {
            id: SourceId::new(one_based).expect("FE source"),
            canonical_name: "g".to_owned(),
        };
        bindings.push(OlsInputBinding {
            stata_var_position: one_based + 2,
            role: OlsInputRole::NumericSource(source.id),
        });
        numeric_sources.push((source.id, loaded(codes)));
        sources.push(source.clone());
        vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("absorbed term"),
            intercept: true,
            categorical_sources: vec![source.id],
            slope_sources: Vec::new(),
            label: "g".to_owned(),
            saved_name: String::new(),
        }]
    });
    let weight_position = u32::try_from(bindings.len() + 1).expect("small weight binding");
    let (analytic_weight, frequency_weight, probability_weight) = match weights {
        TestWeights::Unit => (None, None, None),
        TestWeights::Analytic(values) => {
            bindings.push(OlsInputBinding {
                stata_var_position: weight_position,
                role: OlsInputRole::AnalyticWeight,
            });
            (Some(loaded(values)), None, None)
        }
        TestWeights::Probability(values) => {
            bindings.push(OlsInputBinding {
                stata_var_position: weight_position,
                role: OlsInputRole::ProbabilityWeight,
            });
            (None, None, Some(loaded(values)))
        }
        TestWeights::Frequency(values) => {
            bindings.push(OlsInputBinding {
                stata_var_position: weight_position,
                role: OlsInputRole::FrequencyWeight,
            });
            let values = values
                .iter()
                .map(|value| f64::from(*value))
                .collect::<Vec<_>>();
            (None, Some(loaded(&values)), None)
        }
    };
    (
        OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms,
            bindings,
            include_intercept,
            keep_singletons: true,
        },
        LoadedOlsInput {
            candidate: vec![true; outcome.len()].into(),
            outcome: loaded(outcome),
            numeric_sources,
            analytic_weight,
            frequency_weight,
            probability_weight,
        },
    )
}

fn loaded(values: &[f64]) -> LoadedNumericColumn {
    LoadedNumericColumn {
        values: values.to_vec(),
        missing: vec![false; values.len()],
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    let scale = actual.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance * scale,
        "actual={actual:.17e}, expected={expected:.17e}, tolerance={tolerance:.3e}"
    );
}

fn assert_slice_close(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_close(*actual, *expected, tolerance);
    }
}
