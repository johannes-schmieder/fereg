use super::*;
use crate::{
    data::{DenseMatrix, DropReason, Prepared, PreparedAdjustment, PreparedWeights, WeightKind},
    execution::interruption_tests::every_checkpoint,
    ppml::irls,
    spec::Options,
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    wls::Role,
    CompiledTermGraph, ModelSpec,
};

#[test]
fn ols_statistics_cancel_every_checkpoint_with_unchanged_zero_and_centering_rules() {
    let n = 4099;
    let y: Vec<_> = (0..n).map(|i| (i % 17) as f64 / 2.).collect();
    let wy: Vec<_> = y.iter().map(|y| y - 4.).collect();
    let r: Vec<_> = (0..n).map(|i| (i % 7) as f64 / 8. - 0.375).collect();
    let raw = PreparedWeights::Provided {
        kind: WeightKind::Analytic,
        values: vec![f64::MAX; n],
    };
    let q = ReferenceWeights::new(&raw, n, Estimator::Ols).unwrap();
    let dof = ResidualDof::new(Estimator::Ols, q.effective(), 2, 3, 0, &[]).unwrap();
    for constant in [false, true] {
        let mut reference = None;
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let count = team.install(|| {
                every_checkpoint(|poll| ols_polled(&y, &wy, &r, q, constant, dof, poll))
            });
            assert!(count > 10);
            assert_eq!(*reference.get_or_insert(count), count);
        }
    }
    every_checkpoint(|poll| {
        ols_polled(&vec![0.; n], &vec![0.; n], &vec![0.; n], q, true, dof, poll)
    });
    let zero = ols(&vec![0.; n], &vec![0.; n], &vec![0.; n], q, true, dof).unwrap();
    assert_eq!(
        (zero.rmse, zero.r_squared, zero.log_likelihood),
        (0., None, None)
    );
    let observe = || true;
    let mut invalid = y.clone();
    invalid[1025] = f64::NAN;
    assert_eq!(
        ols(&invalid, &wy, &r, q, true, dof).unwrap_err(),
        ols_polled(&invalid, &wy, &r, q, true, dof, Some(&observe)).unwrap_err()
    );
}

#[test]
fn accepted_ppml_statistics_cancel_every_checkpoint_and_keep_final_state_diagnostics() {
    let n = 2053;
    let sample = Prepared {
        n_original: n,
        n_candidate: n,
        original_row_by_active: (0..n).collect(),
        outcome: (0..n).map(|i| [0., 0.5, 1., 2., 4.][i % 5]).collect(),
        regressors: vec![],
        topology: None,
        weights: PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values: (0..n).map(|i| (1 + i % 3) as f64).collect(),
        },
        adjustment: PreparedAdjustment::Offset(vec![700.; n]),
        drop_reasons: vec![DropReason::Retained; n],
    };
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: vec![],
            terms: vec![],
        },
        absorbed_terms: vec![],
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    let sample = prepare_fixed_effects(&model, sample).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; n]]).unwrap();
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let fit = irls::fit_final_sample(
            &irls::Problem {
                sample: &sample,
                raw: &raw,
                roles: &[Role::Ordinary],
            },
            Options::for_estimator(Estimator::Ppml),
            &team,
        )
        .unwrap();
        let count = team.install(|| every_checkpoint(|poll| ppml_polled(&fit, poll)));
        assert!(count > 20);
        let stats = ppml(&fit).unwrap();
        assert_eq!(stats.deviance, fit.evaluation.objective.deviance);
        assert_eq!(
            stats.log_likelihood,
            fit.evaluation.objective.log_likelihood
        );
        assert_eq!(
            stats.inner_iterations,
            fit.diagnostics.work.counts().summed_iterations
        );
        assert_eq!(*reference.get_or_insert((stats, count)), (stats, count));
    }
}
