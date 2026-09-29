use fereg_engine::{
    data::DenseMatrix,
    fe::Design,
    outputs::{self, Kind, OlsFrame, Request},
    threads::ThreadTeam,
};
pub fn check(
    design: &Design<'_>,
    alpha: &DenseMatrix,
    residuals: &DenseMatrix,
    expected: &[f64],
    team: &ThreadTeam,
) {
    let sample = design.sample();
    let n = design.nobs();
    let finite = DenseMatrix::zeros(n, 0).unwrap();
    let coefficients = DenseMatrix::from_columns(&[alpha.column(0).unwrap()]).unwrap();
    let residual = residuals.column(0).unwrap();
    let constant = if design.topology().effects().iter().any(|e| e.intercept) {
        (0..n)
            .map(|i| design.weights().at(i) * sample.outcome[i])
            .sum::<f64>()
            / (0..n).map(|i| design.weights().at(i)).sum::<f64>()
    } else {
        0.
    };
    let frame = OlsFrame {
        design,
        finite_design: &finite,
        coefficients: &[],
        absorbed_coefficients: &coefficients,
        residuals: &residual,
        reported_constant: constant,
    };
    let request = Request {
        residuals: true,
        xb: true,
        xbd: true,
        absorbed: true,
        drop_reason: true,
        saved: design
            .layout()
            .effects
            .iter()
            .enumerate()
            .flat_map(|(e, l)| (0..l.channels).map(move |c| (e, c)))
            .collect(),
    };
    let result = outputs::ols(&frame, &request, 2e-10, team).unwrap();
    for (row, &expected) in expected.iter().enumerate() {
        assert!(
            (result.get(Kind::Residuals).unwrap()[row] - expected).abs()
                < 2e-9 * expected.abs().max(1.)
        );
        let absorbed = sample.outcome[row] - expected - constant;
        assert!(
            (result.get(Kind::Absorbed).unwrap()[row] - absorbed).abs()
                < 2e-9 * absorbed.abs().max(1.)
        );
        let sum = request
            .saved
            .iter()
            .map(|&(effect, channel)| result.get(Kind::Saved { effect, channel }).unwrap()[row])
            .sum::<f64>();
        assert!((sum - absorbed).abs() < 2e-9 * absorbed.abs().max(1.));
    }
}
