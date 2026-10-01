use super::*;
use crate::{
    data::{DropReason, Prepared, PreparedAdjustment, PreparedWeights, Weights},
    fe::{Design, Plan},
    spec::{Estimator, Options},
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};

fn sample(outcome: Vec<f64>) -> Prepared {
    let source = |id| SourceId::new(id).unwrap();
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=2)
                .map(|id| SourceEntry {
                    id: source(id),
                    canonical_name: format!("g{id}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (1..=2)
            .map(|id| AbsorbedTerm {
                id: AbsorbTermId::new(id).unwrap(),
                categorical_sources: vec![source(id)],
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
            n_original: 4,
            n_candidate: 4,
            original_row_by_active: vec![0, 1, 2, 3],
            outcome,
            regressors: vec![vec![0., 1., 0., 1.], vec![0., 1., 1., 0.]],
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; 4],
        },
    )
    .unwrap()
}

#[test]
fn correction_cannot_accept_zero_work_against_a_mostly_orthogonal_rhs_scale() {
    let weight = 2_f64.powi(-40);
    let delta = 2_f64.powi(-27);
    // Exact binary64 orthogonal contrast plus the identified FE ray [0,0,d,-d].
    let witness = [-8. * weight, -8. * weight, 8., 8.];
    let outcome = vec![witness[0], witness[1], 8. + delta, 8. - delta];
    let sample = sample(outcome.clone());
    let weights = [1., 1., weight, weight];
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let options = Options {
        tolerance: 16. * f64::EPSILON,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let plan = Plan::new(&design, Estimator::Ppml, &options).unwrap();
    let rhs = DenseMatrix::from_columns(&[outcome, witness.to_vec()]).unwrap();
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let mut session = SchurSession::new(&design, &plan, &team).unwrap();
        let candidate = session
            .solve_budget(
                &rhs,
                MonitorControl::Automatic,
                false,
                refinement::Limits {
                    budget: 100,
                    tolerance: options.tolerance,
                    poll: None,
                },
                &team,
            )
            .unwrap();
        let error = candidate
            .residuals
            .column(0)
            .unwrap()
            .iter()
            .zip(witness)
            .fold(0_f64, |v, (a, b)| v.max((a - b).abs()));
        let certificate = design.certify(
            &rhs,
            &candidate.coefficients,
            &candidate.residuals,
            options.tolerance,
            &team,
        );
        eprintln!("Schur exact correction workers={workers}: forward_max={error:.17e}, progress={:?}, certificate={certificate:?}", candidate.progress);
        assert_eq!(candidate.progress[0].stop, StopReason::Converged);
        assert!(
            candidate.progress[0].iterations > 0,
            "nonzero FE correction was accepted without work"
        );
        assert!(
            error <= 1e-9,
            "certified zero-work correction lost the exact FE ray: {error:.17e}"
        );
        assert!(certificate.is_ok());
        assert_eq!(candidate.progress[1].stop, StopReason::Converged);
        assert_eq!(candidate.progress[1].iterations, 0);
        assert_eq!(candidate.residuals.column(1).unwrap(), witness);
        if let Some(prior) = &reference {
            assert_eq!(prior, &candidate);
        } else {
            reference = Some(candidate);
        }
    }
}
