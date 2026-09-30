//! A small original response must retain its score scale after FWL assembly.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::{DenseMatrix, Weights},
    fe::{Design, Engine},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::{self, AbsorbedControls, FrozenBasis, Role, Work},
};
use fereg_oracle::{
    designs::{generate, DesignSpec},
    explicit_ols::{fit_explicit_ols, ExplicitOlsProblem, OracleWeights},
};
#[path = "support/engine.rs"]
mod inputs;

#[test]
fn original_response_score_is_repaired_without_relaxing_the_guard_or_changing_the_basis() {
    let n = 82;
    let effects = generate(
        3,
        &DesignSpec {
            rows: n,
            levels: vec![6, 5, 3],
            regressors: 0,
            ..DesignSpec::default()
        },
    )
    .unwrap()
    .effects;
    let outcome: Vec<_> = (0..n).map(|row| f64::from(row % 7 == 3)).collect();
    // y = -x - Dalpha exactly. The finite coefficient is identified even
    // though its weighted contrast is tiny compared with the FE target.
    let mut column: Vec<_> = outcome.iter().map(|&value| -value).collect();
    for (index, effect) in effects.iter().enumerate() {
        for (row, value) in column.iter_mut().enumerate() {
            *value -= (index + 1 + effect.codes()[row] as usize) as f64 * 0.0625;
        }
    }
    let weights: Vec<_> = outcome
        .iter()
        .map(|&value| {
            if value > 0. {
                1e-5_f64.powi(2) / 12.
            } else {
                1.
            }
        })
        .collect();
    let sample = inputs::prepare(&effects, &outcome);
    let raw = DenseMatrix::from_columns(&[column.clone()]).unwrap();
    let options = Options {
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let policy = wls::Policy {
        score_tolerance: 1e-10,
        ..wls::Policy::default()
    };
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: outcome.clone(),
        regressors: vec![column],
        effects,
        weights: OracleWeights::Wls(weights.clone()),
        clusters: vec![],
    })
    .unwrap();
    let unit = Design::new(&sample, Weights::Unit).unwrap();
    let one = ThreadTeam::new(1).unwrap();
    let mut unit_engine = Engine::new(&unit, Estimator::Ppml, &options, &one).unwrap();
    let canonical = wls::solve_absorbed(
        &mut unit_engine,
        &raw,
        &vec![0.; n],
        &[Role::Ordinary],
        policy,
        &one,
    )
    .unwrap();
    let basis = FrozenBasis::from_fit(&canonical.finite).unwrap();
    assert_eq!(basis.kept(), [0]);
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let mut reference = None;
    for workers in [1, 2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let mut engine = Engine::new(&design, Estimator::Ppml, &options, &team).unwrap();
        let mut work = Work::default();
        let fit = wls::solve_absorbed_recorded(
            &mut engine,
            &raw,
            &outcome,
            &[Role::Ordinary],
            AbsorbedControls {
                finite: policy,
                basis: Some(&basis),
            },
            &mut work,
            &team,
        )
        .unwrap();
        assert!(basis.matches(&fit.finite));
        assert!(work.projection().counts().refinements > 0);
        assert!(fit.model_refinements <= 3);
        assert!(fit.relative_original_finite_score <= policy.score_tolerance);
        assert!(fit.model_certificate.relative_score[0] <= options.tolerance);
        assert!((fit.finite.coefficients[0] + 1.).abs() <= 1e-10);
        assert!((fit.finite.coefficients[0] - oracle.coefficients[0].unwrap()).abs() <= 1e-10);
        // The binary-exact construction proves the true residual is zero.
        // Dense weighted QR loses digits on downweighted rows; this exact
        // witness, rather than that oracle residual, controls forward accuracy.
        for (row, &actual) in fit.residuals.iter().enumerate() {
            assert!(actual.abs() <= 1e-12, "row={row}, actual={actual:e}");
        }
        assert_eq!(work.projection().calls(), 1);
        assert_eq!(work.recovery().calls(), fit.recovery_diagnostics.len());
        assert!(work.recovery().calls() >= fit.model_refinements);
        if let Some((previous, previous_work)) = &reference {
            assert_eq!(previous, &fit);
            assert_eq!(previous_work, &work);
        } else {
            reference = Some((fit, work));
        }
    }
}
