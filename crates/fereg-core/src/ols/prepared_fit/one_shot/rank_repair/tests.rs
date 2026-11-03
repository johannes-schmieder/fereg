use super::*;
use crate::fe::{BorrowedInterceptEffect, LevelIndex};

fn with_design(codes: &[Vec<u32>], rows: usize, visit: impl FnOnce(BorrowedInterceptFeDesign<'_>)) {
    let levels: Vec<_> = codes
        .iter()
        .map(|c| c.iter().max().copied().unwrap_or(0) as usize + 1)
        .collect();
    let indices: Vec<_> = codes
        .iter()
        .zip(&levels)
        .map(|(c, &n)| LevelIndex::from_codes(c, n).unwrap())
        .collect();
    let effects = codes
        .iter()
        .zip(&levels)
        .zip(&indices)
        .map(|((c, &n), index)| BorrowedInterceptEffect::new(c, n, index).unwrap())
        .collect();
    visit(BorrowedInterceptFeDesign::new(rows, effects).unwrap());
}

fn unit() -> NormalizedWeights<'static> {
    NormalizedWeights {
        values: None,
        raw_scale: 1.,
        kind: None,
    }
}

#[test]
fn exact_single_partition_proof_precedes_rounded_joint_gauges() {
    let codes = vec![vec![0, 0, 1, 1], vec![0, 1, 0, 1]];
    with_design(&codes, 4, |design| {
        assert!(
            structurally_absorbed(&design, |row| Ok([0.1, 0.1, -0.7, -0.7][row]))
                .unwrap()
                .is_some()
        );
        assert!(
            structurally_absorbed(&design, |row| Ok([0.1, -0.7, 0.1, -0.7][row]))
                .unwrap()
                .is_some()
        );
        assert!(
            structurally_absorbed(&design, |row| Ok([0.1, 0.2, -0.7, -0.7][row]))
                .unwrap()
                .is_none()
        );
    });
    with_design(&[], 4, |design| {
        assert!(structurally_absorbed(&design, |row| Ok([0.1; 4][row]))
            .unwrap()
            .is_some());
        assert!(
            structurally_absorbed(&design, |row| Ok([0.1, 0.1, 0.1, 0.2][row]))
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn compensated_cycle_preserves_signal_lost_by_sequential_subtraction() {
    let huge = 2.0_f64.powi(54);
    assert_eq!((1. + huge) - huge, 0.);
    let codes = vec![vec![0, 0, 1, 1], vec![0, 1, 0, 1]];
    with_design(&codes, 4, |design| {
        let mut forest = Forest::new(&design, &mut SystemFitReservation).unwrap();
        let deflated = forest
            .deflate(vec![huge, huge, 1., 0.], unit(), &mut SystemFitReservation)
            .unwrap();
        assert_eq!(deflated.values.as_slice(), &[0., 0., 1., 0.]);
        assert_eq!(deflated.roundoff_bound, 0.);
        assert!(!deflated.exact_absorbed);
        assert_eq!(deflated.gauge, vec![0., -huge, huge, huge]);
        let rebuilt = forest
            .reconstruct(
                &[1., 0., 1., 0.],
                &[huge, huge, -huge, -huge],
                unit(),
                &mut SystemFitReservation,
            )
            .unwrap();
        assert_eq!(rebuilt.values.as_slice(), &[1., 0., 1., 0.]);
        assert_eq!(rebuilt.roundoff_bound, 0.);
    });
}

#[test]
fn exact_joint_dependence_and_large_resolved_shifts_are_distinguished() {
    let codes = vec![vec![0, 0, 1, 1], vec![0, 1, 0, 1]];
    with_design(&codes, 4, |design| {
        let mut forest = Forest::new(&design, &mut SystemFitReservation).unwrap();
        let dependent = vec![13., 17., 23., 27.];
        assert!(structurally_absorbed(&design, |row| Ok(dependent[row]))
            .unwrap()
            .is_none());
        let result = forest
            .deflate(dependent, unit(), &mut SystemFitReservation)
            .unwrap();
        assert!(result.exact_absorbed);
        assert_eq!(result.roundoff_bound, 0.);
        for scale in [1e4, 1e6, 1e8] {
            let raw = vec![scale + 1., scale - 1., 2. * scale - 1., 2. * scale + 1.];
            let result = forest
                .deflate(raw, unit(), &mut SystemFitReservation)
                .unwrap();
            assert!(!result.exact_absorbed);
            assert_eq!(result.roundoff_bound, 0.);
            assert_eq!(result.values.as_slice(), &[0., 0., 0., 4.]);
        }
    });
}

#[test]
fn weighted_disconnected_cycle_bound_dominates_analytic_projection() {
    let codes = vec![vec![0, 0, 1, 1, 2, 2, 3, 3], vec![0, 1, 0, 1, 2, 3, 2, 3]];
    let raw = [2., -1., 3., 7., -4., 2., 5., -3.];
    let weights = [1., 2., 4., 8., 8., 4., 2., 1.];
    with_design(&codes, 8, |design| {
        let mut forest = Forest::new(&design, &mut SystemFitReservation).unwrap();
        let frame = NormalizedWeights {
            values: Some(&weights),
            raw_scale: 1.,
            kind: Some(WeightKind::Analytic),
        };
        let bound = forest.flow_error(&raw, frame).unwrap();
        // On each 2x2 component the orthogonal complement is c_i/w_i,
        // c=(1,-1,-1,1). Pythagoras gives the exact projection energy.
        let mut energy = 0.;
        for component in 0..2 {
            let start = component * 4;
            let total: f64 = (start..start + 4)
                .map(|i| weights[i] * raw[i] * raw[i])
                .sum();
            let contrast = raw[start] - raw[start + 1] - raw[start + 2] + raw[start + 3];
            let inverse_mass: f64 = weights[start..start + 4].iter().map(|w| 1. / w).sum();
            energy += total - contrast * contrast / inverse_mass;
        }
        assert!(bound >= energy.sqrt());
        let repeat = forest.flow_error(&raw, frame).unwrap();
        assert_eq!(bound.to_bits(), repeat.to_bits());
        let orthogonal = [1., -0.5, -0.25, 0.125, 0.125, -0.25, -0.5, 1.];
        assert!(forest.flow_error(&orthogonal, frame).unwrap() < 1e-300);
    });
}

#[test]
fn global_and_single_effect_flow_bounds_cover_closed_form_projection() {
    let raw = [1., 3., -2., 4.];
    let weights = [1., 2., 4., 8.];
    let frame = NormalizedWeights {
        values: Some(&weights),
        raw_scale: 1.,
        kind: Some(WeightKind::Analytic),
    };
    for codes in [vec![], vec![vec![0, 0, 1, 1]]] {
        with_design(&codes, 4, |design| {
            let mut forest = Forest::new(&design, &mut SystemFitReservation).unwrap();
            let energy: f64 = if codes.is_empty() {
                31. * 31. / 15.
            } else {
                7. * 7. / 3. + 24. * 24. / 12.
            };
            assert!(forest.flow_error(&raw, frame).unwrap() >= energy.sqrt());
            let shifted = forest
                .deflate(raw.to_vec(), frame, &mut SystemFitReservation)
                .unwrap();
            assert_eq!(shifted.gauge.len(), if codes.is_empty() { 1 } else { 2 });
            assert_eq!(shifted.roundoff_bound, 0.);
        });
    }
}

#[test]
fn nonfinite_gauge_and_reconstruction_refuse_without_false_zero() {
    let codes = vec![vec![0, 0, 1, 1], vec![0, 1, 0, 1]];
    with_design(&codes, 4, |design| {
        let mut forest = Forest::new(&design, &mut SystemFitReservation).unwrap();
        assert!(forest
            .deflate(
                vec![f64::MAX, 0., -f64::MAX, 0.],
                unit(),
                &mut SystemFitReservation
            )
            .is_err());
        assert!(forest
            .reconstruct(
                &[1.; 4],
                &[f64::INFINITY; 4],
                unit(),
                &mut SystemFitReservation
            )
            .is_err());
    });
}
