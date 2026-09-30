//! Independent acceptance and atomic gauges must survive every cancellation.
use super::Design;
use crate::{
    data::{DenseMatrix, DropReason, Prepared, PreparedAdjustment, PreparedWeights, Weights},
    execution::Poll,
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ErrorCode, ModelSpec, Result, SourceEntry,
    SourceId,
};
use std::sync::atomic::{AtomicUsize, Ordering};

fn sample() -> Prepared {
    let sources = (1..=4)
        .map(|i| SourceEntry {
            id: SourceId::new(i).unwrap(),
            canonical_name: format!("s{i}"),
        })
        .collect();
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources,
            terms: vec![],
        },
        absorbed_terms: (1..=2)
            .map(|i| AbsorbedTerm {
                id: AbsorbTermId::new(i).unwrap(),
                intercept: true,
                categorical_sources: vec![SourceId::new(i).unwrap()],
                slope_sources: if i == 1 {
                    vec![SourceId::new(3).unwrap(), SourceId::new(4).unwrap()]
                } else {
                    vec![]
                },
                label: format!("g{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    // Two disconnected components, each longer than one reduction block;
    // the first effect has two exactly collinear declared slope channels.
    let n = 2053;
    let codes: Vec<_> = (0..n).map(|i| (i % 2) as f64).collect();
    let slope: Vec<_> = (0..n).map(|i| (i % 7) as f64 - 3.).collect();
    prepare_fixed_effects(
        &model,
        Prepared {
            topology: None,
            n_original: n,
            n_candidate: n,
            original_row_by_active: (0..n).collect(),
            outcome: vec![1.; n],
            regressors: vec![
                codes.clone(),
                codes,
                slope.clone(),
                slope.iter().map(|x| 2. * x).collect(),
            ],
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; n],
        },
    )
    .unwrap()
}
fn every_checkpoint<T: std::fmt::Debug + PartialEq>(
    mut run: impl FnMut(Poll<'_>) -> Result<T>,
) -> usize {
    let expected = run(None).unwrap();
    let count = AtomicUsize::new(0);
    let observe = || {
        count.fetch_add(1, Ordering::Relaxed);
        true
    };
    assert_eq!(run(Some(&observe)).unwrap(), expected);
    let total = count.load(Ordering::Relaxed);
    assert!(total > 2);
    for stop in 1..=total {
        count.store(0, Ordering::Relaxed);
        let cancel = || {
            count
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    (value < stop).then_some(value + 1)
                })
                .is_ok_and(|previous| previous + 1 < stop)
        };
        assert_eq!(run(Some(&cancel)).unwrap_err().code, ErrorCode::Interrupted);
        assert_eq!(count.load(Ordering::Relaxed), stop);
        assert_eq!(run(None).unwrap(), expected);
    }
    total
}

#[test]
fn fresh_span_certification_aborts_every_checkpoint_and_keeps_worker_identity() {
    let sample = sample();
    let weights: Vec<_> = (0..sample.n_active())
        .map(|i| 0.5 + (i % 5) as f64 / 4.)
        .collect();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let mut coefficients = DenseMatrix::zeros(design.layout().total, 2).unwrap();
    for level in 0..2 {
        assert_eq!(
            super::certificate_basis::level_basis(&design, 0, level, None)
                .unwrap()
                .len(),
            2
        );
    }
    for (i, value) in coefficients.as_mut_slice().iter_mut().enumerate() {
        *value = (i % 7) as f64 / 8. - 0.25;
    }
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let rhs = design.multiply(&coefficients, &team).unwrap();
        let residual = DenseMatrix::zeros(sample.n_active(), rhs.ncols()).unwrap();
        every_checkpoint(|poll| design.multiply_polled(&coefficients, &team, poll));
        every_checkpoint(|poll| design.transpose_weighted_polled(&rhs, &team, poll));
        every_checkpoint(|poll| design.reconstruct_polled(&rhs, &coefficients, &team, poll));
        let total = every_checkpoint(|poll| {
            design.certify_polled(&rhs, &coefficients, &residual, 1e-10, &team, poll)
        });
        let certified = design
            .certify(&rhs, &coefficients, &residual, 1e-10, &team)
            .unwrap();
        assert_eq!(certified.relative_score, vec![0.; 2]);
        let state = (total, certified);
        if let Some(reference) = &reference {
            assert_eq!(reference, &state);
        } else {
            reference = Some(state);
        }
    }
}
#[test]
fn disconnected_sloped_gauges_abort_without_mutating_the_original_coefficients() {
    let sample = sample();
    let weights: Vec<_> = (0..sample.n_active())
        .map(|i| 0.5 + (i % 5) as f64 / 4.)
        .collect();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let mut original = DenseMatrix::zeros(design.layout().total, 2).unwrap();
    for (i, value) in original.as_mut_slice().iter_mut().enumerate() {
        *value = (i % 7) as f64 / 8. - 0.25;
    }
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let total = every_checkpoint(|poll| {
            let mut coefficients = original.clone();
            let result =
                design.normalize_coefficients_polled(&mut coefficients, 1e-10, &team, poll);
            if result.is_err() {
                assert_eq!(coefficients, original);
            }
            result.map(|()| coefficients)
        });
        let mut normalized = original.clone();
        design
            .normalize_coefficients(&mut normalized, 1e-10, &team)
            .unwrap();
        for column in 0..2 {
            for level in 0..2 {
                assert_eq!(normalized[(6 + level, column)], 0.);
            }
            for slope in 2..6 {
                assert_eq!(normalized[(slope, column)], original[(slope, column)]);
            }
        }
        assert_eq!(
            design.multiply(&normalized, &team).unwrap(),
            design.multiply(&original, &team).unwrap()
        );
        let state = (total, normalized);
        if let Some(reference) = &reference {
            assert_eq!(reference, &state);
        } else {
            reference = Some(state);
        }
    }
}
