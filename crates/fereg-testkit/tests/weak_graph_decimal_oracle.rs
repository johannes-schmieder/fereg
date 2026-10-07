use fereg_core::{absorb_lsmr, DenseMatrix, EffectTerm, FeDesign, LsmrConfig, LsmrPreconditioner};
use fereg_testkit::explicit_dummy::fe_remaining_projection_with_reference;

const ORACLE_CSV: &str = include_str!("../../../tests/reference/weak_graph_decimal_oracle_v1.csv");
const ATOL: f64 = 1e-9;
const RTOL: f64 = 1e-7;
const REQUESTED_TOLERANCE: f64 = 1e-8;
const CANDIDATE_TOLERANCE: f64 = 1e-10;
const STABILITY_TOLERANCE: f64 = 1e-12;

#[derive(Debug)]
struct OracleFixture {
    id: String,
    weights: Vec<f64>,
    effect_codes: [Vec<u32>; 2],
    effect_intercepts: [bool; 2],
    effect_slopes: [Vec<f64>; 2],
    inputs: Vec<Vec<f64>>,
    fitted: Vec<Vec<f64>>,
    residuals: Vec<Vec<f64>>,
    objectives: Vec<f64>,
}

fn parse_optional_f64(value: &str) -> Option<f64> {
    (!value.is_empty()).then(|| value.parse::<f64>().expect("oracle f64"))
}

fn parse_fixtures() -> Vec<OracleFixture> {
    let mut fixtures: Vec<OracleFixture> = Vec::new();
    for (line_number, line) in ORACLE_CSV.lines().enumerate().skip(1) {
        let fields = line.split(',').collect::<Vec<_>>();
        assert_eq!(fields.len(), 17, "oracle CSV line {}", line_number + 1);
        let id = fields[0];
        if fixtures.last().is_none_or(|fixture| fixture.id != id) {
            fixtures.push(OracleFixture {
                id: id.to_owned(),
                weights: Vec::new(),
                effect_codes: [Vec::new(), Vec::new()],
                effect_intercepts: [fields[4] == "1", fields[7] == "1"],
                effect_slopes: [Vec::new(), Vec::new()],
                inputs: vec![Vec::new()],
                fitted: vec![Vec::new()],
                residuals: vec![Vec::new()],
                objectives: vec![fields[12].parse().expect("rhs0 objective")],
            });
            if !fields[13].is_empty() {
                let fixture = fixtures.last_mut().expect("new fixture");
                fixture.inputs.push(Vec::new());
                fixture.fitted.push(Vec::new());
                fixture.residuals.push(Vec::new());
                fixture
                    .objectives
                    .push(fields[16].parse().expect("rhs1 objective"));
            }
        }
        let fixture = fixtures.last_mut().expect("fixture");
        assert_eq!(
            fields[1].parse::<usize>().expect("observation"),
            fixture.weights.len()
        );
        fixture.weights.push(fields[2].parse().expect("weight"));
        fixture.effect_codes[0].push(fields[3].parse().expect("effect0 code"));
        fixture.effect_codes[1].push(fields[6].parse().expect("effect1 code"));
        if let Some(slope) = parse_optional_f64(fields[5]) {
            fixture.effect_slopes[0].push(slope);
        }
        if let Some(slope) = parse_optional_f64(fields[8]) {
            fixture.effect_slopes[1].push(slope);
        }
        fixture.inputs[0].push(fields[9].parse().expect("rhs0 input"));
        fixture.fitted[0].push(fields[10].parse().expect("rhs0 fitted"));
        fixture.residuals[0].push(fields[11].parse().expect("rhs0 residual"));
        assert_eq!(
            fixture.objectives[0],
            fields[12].parse::<f64>().expect("repeated rhs0 objective")
        );
        if fixture.inputs.len() == 2 {
            fixture.inputs[1].push(fields[13].parse().expect("rhs1 input"));
            fixture.fitted[1].push(fields[14].parse().expect("rhs1 fitted"));
            fixture.residuals[1].push(fields[15].parse().expect("rhs1 residual"));
            assert_eq!(
                fixture.objectives[1],
                fields[16].parse::<f64>().expect("repeated rhs1 objective")
            );
        }
    }
    fixtures
}

fn design(fixture: &OracleFixture) -> FeDesign {
    let effects = (0..2)
        .map(|effect| {
            let codes = fixture.effect_codes[effect].clone();
            let levels = codes.iter().copied().max().expect("nonempty codes") as usize + 1;
            let slopes = if fixture.effect_slopes[effect].is_empty() {
                Vec::new()
            } else {
                vec![fixture.effect_slopes[effect].clone()]
            };
            EffectTerm::new(codes, levels, fixture.effect_intercepts[effect], slopes)
                .expect("oracle effect")
        })
        .collect::<Vec<_>>();
    FeDesign::new(fixture.weights.len(), effects).expect("oracle design")
}

fn assert_envelope(actual: f64, expected: f64, label: &str) {
    let error = (actual - expected).abs();
    let limit = ATOL + RTOL * expected.abs();
    assert!(
        error <= limit,
        "{label}: actual={actual:e}, expected={expected:e}, error={error:e}, limit={limit:e}"
    );
}

fn solve(fixture: &OracleFixture, tolerance: f64) -> (DenseMatrix, fereg_core::SolveReport) {
    let mut matrix = DenseMatrix::from_columns(&fixture.inputs).expect("input matrix");
    let report = absorb_lsmr(
        &design(fixture),
        &fixture.weights,
        &mut matrix,
        LsmrConfig {
            tolerance,
            preconditioner: LsmrPreconditioner::Schwarz,
            ..LsmrConfig::default()
        },
    )
    .expect("direct Schwarz LSMR");
    (matrix, report)
}

#[test]
fn direct_schwarz_lsmr_matches_the_sealed_decimal_oracle() {
    let fixtures = parse_fixtures();
    assert_eq!(fixtures.len(), 6);
    for fixture in &fixtures {
        let reference = DenseMatrix::from_columns(&fixture.inputs).expect("reference");
        let (candidate, report) = solve(fixture, CANDIDATE_TOLERANCE);
        assert!(report.certified(), "{}: {report:?}", fixture.id);
        for rhs in 0..candidate.ncols() {
            let mut objective = 0.0_f64;
            for row in 0..candidate.nrows() {
                let residual = candidate[(row, rhs)];
                let fitted = fixture.inputs[rhs][row] - residual;
                assert_envelope(
                    residual,
                    fixture.residuals[rhs][row],
                    &format!("{} rhs{rhs} row{row} residual", fixture.id),
                );
                assert_envelope(
                    fitted,
                    fixture.fitted[rhs][row],
                    &format!("{} rhs{rhs} row{row} fitted", fixture.id),
                );
                objective = fixture.weights[row].mul_add(residual * residual, objective);
            }
            assert_envelope(
                objective,
                fixture.objectives[rhs],
                &format!("{} rhs{rhs} objective", fixture.id),
            );
        }
        let audit = fe_remaining_projection_with_reference(
            &design(fixture),
            &fixture.weights,
            &candidate,
            &reference,
        )
        .expect("observation-space audit");
        assert!(
            audit.maximum_relative_two_norm <= REQUESTED_TOLERANCE,
            "{}: {audit:?}",
            fixture.id
        );
    }
}

#[test]
fn candidate_is_deterministic_and_stable_against_a_hundredfold_tighter_solve() {
    for fixture in parse_fixtures() {
        let (candidate, candidate_report) = solve(&fixture, CANDIDATE_TOLERANCE);
        let (repeated, repeated_report) = solve(&fixture, CANDIDATE_TOLERANCE);
        assert_eq!(candidate, repeated, "{} residual determinism", fixture.id);
        assert_eq!(candidate_report.iterations, repeated_report.iterations);

        let (tight, tight_report) = solve(&fixture, STABILITY_TOLERANCE);
        assert!(tight_report.certified(), "{}: {tight_report:?}", fixture.id);
        for (index, (&actual, &reference)) in candidate
            .as_slice()
            .iter()
            .zip(tight.as_slice())
            .enumerate()
        {
            assert_envelope(
                actual,
                reference,
                &format!("{} stability {index}", fixture.id),
            );
        }
    }
}
