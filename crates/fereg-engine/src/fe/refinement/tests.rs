use super::*;
use crate::{
    data::{DropReason, Prepared, PreparedAdjustment, PreparedWeights, Weights},
    fe::Backend,
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};

fn sample() -> Prepared {
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=2)
                .map(|id| SourceEntry {
                    id: SourceId::new(id).unwrap(),
                    canonical_name: format!("s{id}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (1..=2)
            .map(|id| AbsorbedTerm {
                id: AbsorbTermId::new(id).unwrap(),
                categorical_sources: vec![SourceId::new(id).unwrap()],
                slope_sources: vec![],
                intercept: true,
                label: format!("g{id}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    prepare_fixed_effects(
        &model,
        Prepared {
            topology: None,
            n_original: 2,
            n_candidate: 2,
            original_row_by_active: vec![0, 1],
            outcome: vec![1.01 + 1e-13, 0.99 + 1e-13],
            regressors: vec![vec![0.; 2]; 2],
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; 2],
        },
    )
    .unwrap()
}
fn attempt() -> Attempt {
    Attempt {
        refinements: 0,
        backend: Backend::Lsmr,
        iterations: 1,
        stop: StopReason::Converged,
    }
}
fn refine(
    design: &Design<'_>,
    bank: &mut DenseMatrix,
    budget: usize,
    tolerance: f64,
    team: &ThreadTeam,
) -> (Attempt, Vec<Measurement>) {
    let rhs = DenseMatrix::from_columns(&[design.sample().outcome.clone()]).unwrap();
    let mut reports = vec![attempt()];
    let mut measurements = Vec::new();
    run_observed(
        design,
        &rhs,
        (bank, &mut reports),
        Limits {
            budget,
            tolerance,
            poll: None,
        },
        team,
        |residual, remaining, _, _| {
            assert!(remaining > 0);
            // Independent projection onto the two identical intercept spans.
            let mean = (residual[(0, 0)] + residual[(1, 0)]) / 2.;
            Ok((
                DenseMatrix::from_columns(&[vec![mean, 0.]]).unwrap(),
                vec![attempt()],
            ))
        },
        |point, _| measurements.push(point),
    )
    .unwrap();
    (reports.remove(0), measurements)
}

#[test]
fn rounded_away_correction_requires_the_unchanged_original_score() {
    let sample = sample();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let large = 2_f64.powi(40);
    let original = DenseMatrix::from_columns(&[vec![large, 1. - large]]).unwrap();
    let mut expected = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let mut bank = original.clone();
        let (accepted, points) = refine(&design, &mut bank, 10, 1e-12, &team);
        assert!(points[0].score < 1e-12 && points[0].score > 1e-14);
        let proposed = points[0].score * points[0].reference_norm / points[0].within_norm;
        assert!(proposed > points[0].update_target);
        assert_eq!(bank, original, "the coefficient increment must round away");
        assert_eq!(accepted.stop, StopReason::Converged);
        assert_eq!(accepted.refinements, 1);
        assert_eq!(points[1].update, 0.);
        let (rejected, _) = refine(&design, &mut bank, 10, 1e-14, &team);
        assert_eq!(rejected.stop, StopReason::Stagnation);
        assert_eq!(rejected.refinements, 1);
        assert_eq!(
            *expected.get_or_insert((accepted.clone(), rejected.clone(), points.clone())),
            (accepted, rejected, points)
        );
    }
}

#[test]
fn representable_correction_needs_stability_and_charges_the_same_budget() {
    let sample = sample();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let mut limited = DenseMatrix::zeros(2, 1).unwrap();
    let (report, points) = refine(&design, &mut limited, 2, 1e-12, &team);
    assert_eq!(report.stop, StopReason::BudgetExhausted);
    assert_eq!(report.iterations, 2);
    assert_eq!(report.refinements, 1);
    assert!(points[1].score <= 1e-12);
    assert!(points[1].update > points[1].update_target);
    let mut complete = DenseMatrix::zeros(2, 1).unwrap();
    let (report, points) = refine(&design, &mut complete, 10, 1e-12, &team);
    assert_eq!(report.stop, StopReason::Converged);
    assert_eq!(report.iterations, 3);
    assert_eq!(report.refinements, 2);
    assert_eq!(points.last().unwrap().update, 0.);
    assert_eq!(complete, limited);
}
