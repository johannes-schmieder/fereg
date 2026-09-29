use fereg_engine::{
    data::*,
    fe::{Design, Weights},
    ppml::{
        family::Family,
        step::{Accepted, Point},
    },
    spec::Estimator,
    threads::ThreadTeam,
    ErrorCode,
};
mod support;

fn point(fe: &Design<'_>, raw: &DenseMatrix, offset: &[f64]) -> Point {
    let finite = vec![0.2, -0.1];
    let absorbed = DenseMatrix::from_columns(&[(0..fe.layout().total)
        .map(|i| i as f64 * 0.02 - 0.1)
        .collect()])
    .unwrap();
    let eta = (0..fe.nobs())
        .map(|row| {
            let mut d = 0.;
            for (effect, layout) in fe.layout().effects.iter().enumerate() {
                let level = fe.topology().effect_partition(effect).unwrap().codes()[row] as usize;
                for channel in 0..layout.channels {
                    d += fe.basis_value(effect, channel, row).unwrap()
                        * absorbed[(layout.offset + channel * layout.levels + level, 0)];
                }
            }
            offset[row] + finite[0] * raw[(row, 0)] + finite[1] + d
        })
        .collect();
    Point {
        finite,
        absorbed,
        eta,
    }
}
#[test]
fn original_sloped_multiway_model_reconstructs_with_offsets_at_every_worker_count() {
    let sample = support::prepared(&[3, 2], true, 8, 0);
    let n = sample.n_active();
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let raw =
        DenseMatrix::from_columns(&[(0..n).map(|i| (i % 11) as f64 / 5.).collect(), vec![1.; n]])
            .unwrap();
    let offset: Vec<_> = (0..n).map(|i| ((i % 5) as f64 - 2.) * 0.1).collect();
    let p = point(&fe, &raw, &offset);
    let q = ReferenceWeights::new(&sample.weights, n, Estimator::Ppml).unwrap();
    let family = Family::new(&sample.outcome, q, Some(&offset)).unwrap();
    let accepted = Accepted::start(&family, p, &ThreadTeam::new(1).unwrap()).unwrap();
    let mut baseline = None;
    for workers in [1, 2, 4, 8] {
        let cert = accepted
            .certify_state(&raw, &fe, 1e-12, &ThreadTeam::new(workers).unwrap())
            .unwrap();
        assert!(cert.predictor_relative_max < 1e-15);
        assert_eq!(cert.mean_relative_max, 0.);
        assert_eq!(cert.deviance_relative_error, 0.);
        assert_eq!(cert.likelihood_relative_error, 0.);
        assert_eq!(*baseline.get_or_insert(cert), cert);
    }
}
#[test]
fn zero_scores_cannot_certify_coefficients_that_do_not_generate_the_predictor() {
    let sample = support::prepared(&[2], false, 2, 0);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; 4]]).unwrap();
    let q = ReferenceWeights::new(&sample.weights, 4, Estimator::Ppml).unwrap();
    let family = Family::new(&sample.outcome, q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let mut p = Point {
        finite: vec![0.],
        absorbed: DenseMatrix::zeros(2, 1).unwrap(),
        eta: vec![0.; 4],
    };
    let evaluation = family.evaluate(&p.eta, &team).unwrap();
    assert!(
        family
            .score_certificate(&p.eta, &raw, &fe, 1e-12, &team)
            .unwrap()
            .passed
    );
    family
        .certify_state(&p, &evaluation, &raw, &fe, 1e-12, &team)
        .unwrap();
    p.finite[0] = 0.01;
    assert_eq!(
        family
            .certify_state(&p, &evaluation, &raw, &fe, 1e-12, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    p.finite[0] = 0.;
    p.absorbed[(0, 0)] = 0.01;
    assert!(family
        .certify_state(&p, &evaluation, &raw, &fe, 1e-12, &team)
        .is_err());
}
#[test]
fn stored_mean_deviance_likelihood_and_family_inputs_are_replayed() {
    let sample = support::prepared(&[2], true, 2, 0);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![0., 1., 2., 3.], vec![1.; 4]]).unwrap();
    let offset = [0.1, 0.2, -0.2, 0.3];
    let p = point(&fe, &raw, &offset);
    let q = ReferenceWeights::new(&sample.weights, 4, Estimator::Ppml).unwrap();
    let family = Family::new(&sample.outcome, q, Some(&offset)).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    for mutation in 0..3 {
        let mut evaluation = family.evaluate(&p.eta, &team).unwrap();
        match mutation {
            0 => evaluation.mean[0] += 0.1,
            1 => evaluation.objective.deviance += 0.1,
            _ => evaluation.objective.log_likelihood += 0.1,
        }
        assert_eq!(
            family
                .certify_state(&p, &evaluation, &raw, &fe, 1e-12, &team)
                .unwrap_err()
                .code,
            ErrorCode::NonConvergence
        );
    }
    let evaluation = family.evaluate(&p.eta, &team).unwrap();
    let wrong_offset = Family::new(&sample.outcome, q, None).unwrap();
    assert!(wrong_offset
        .certify_state(&p, &evaluation, &raw, &fe, 1e-12, &team)
        .is_err());
    let wrong_y = Family::new(&[3.; 4], q, Some(&offset)).unwrap();
    assert!(wrong_y
        .certify_state(&p, &evaluation, &raw, &fe, 1e-12, &team)
        .is_err());
}
#[test]
fn no_fe_and_no_finite_columns_share_the_same_reconstruction() {
    let team = ThreadTeam::new(1).unwrap();
    let sample = support::prepared(&[], false, 1, 0);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let q = ReferenceWeights::new(&sample.weights, 4, Estimator::Ppml).unwrap();
    for eta in [-744., -700., 0., 700.] {
        let offset = [eta; 4];
        let family = Family::new(&sample.outcome, q, Some(&offset)).unwrap();
        let p = Point {
            finite: vec![],
            absorbed: DenseMatrix::zeros(0, 1).unwrap(),
            eta: offset.to_vec(),
        };
        let evaluation = family.evaluate(&p.eta, &team).unwrap();
        let cert = family
            .certify_state(
                &p,
                &evaluation,
                &DenseMatrix::zeros(4, 0).unwrap(),
                &fe,
                1e-12,
                &team,
            )
            .unwrap();
        assert_eq!(cert.predictor_relative_max, 0.);
        assert_eq!(cert.mean_relative_max, 0.);
    }
}
#[test]
fn malformed_frames_and_unrepresentable_reconstruction_fail_without_publication() {
    let sample = support::prepared(&[], false, 1, 0);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let q = ReferenceWeights::new(&sample.weights, 4, Estimator::Ppml).unwrap();
    let family = Family::new(&sample.outcome, q, None).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let mut raw = DenseMatrix::from_columns(&[vec![1.; 4]]).unwrap();
    let mut p = Point {
        finite: vec![0.],
        absorbed: DenseMatrix::zeros(0, 1).unwrap(),
        eta: vec![0.; 4],
    };
    let mut evaluation = family.evaluate(&p.eta, &team).unwrap();
    for tolerance in [0., -1., 1., f64::NAN] {
        assert!(family
            .certify_state(&p, &evaluation, &raw, &fe, tolerance, &team)
            .is_err());
    }
    evaluation.mean[0] = 0.;
    assert!(family
        .certify_state(&p, &evaluation, &raw, &fe, 1e-12, &team)
        .is_err());
    evaluation.mean[0] = 1.;
    p.finite[0] = 1e308;
    raw[(0, 0)] = 2.;
    assert_eq!(
        family
            .certify_state(&p, &evaluation, &raw, &fe, 1e-12, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    p.finite.clear();
    assert_eq!(
        family
            .certify_state(&p, &evaluation, &raw, &fe, 1e-12, &team)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}
