#![deny(clippy::too_many_lines)]
//! Parser harvested from the legacy weak-graph test; fixture bytes are unchanged.
use fereg_oracle::compare::PREDICTION;
use fereg_oracle::explicit_dummy::{
    design::ExplicitFeDesign as FeDesign, fe_remaining_projection_with_reference,
};
use fereg_oracle::explicit_ols::{
    fit_explicit_ols, DummyEffect as EffectTerm, ExplicitOlsProblem, OracleMatrix as DenseMatrix,
    OracleWeights,
};
const ORACLE_CSV: &str = include_str!("../../../tests/reference/weak_graph_decimal_oracle_v1.csv");
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

#[test]
fn dense_oracles_match_the_preserved_hundred_digit_weak_graph_fixtures() {
    let fixtures = parse_fixtures();
    assert_eq!(fixtures.len(), 6);
    for fixture in fixtures {
        let design = design(&fixture);
        for rhs in 0..fixture.inputs.len() {
            // Duplicate each row so saturated projection fixtures also admit OLS inference.
            let duplicate =
                |values: &[f64]| values.iter().chain(values).copied().collect::<Vec<_>>();
            let effects = design
                .effects()
                .iter()
                .map(|e| {
                    EffectTerm::new(
                        e.codes().iter().chain(e.codes()).copied().collect(),
                        e.n_levels(),
                        e.has_intercept(),
                        e.slopes().iter().map(|s| duplicate(s)).collect(),
                    )
                    .unwrap()
                })
                .collect();
            let problem = ExplicitOlsProblem {
                outcome: duplicate(&fixture.inputs[rhs]),
                regressors: vec![],
                effects,
                weights: OracleWeights::Wls(duplicate(&fixture.weights)),
                clusters: vec![],
            };
            let fit = fit_explicit_ols(&problem).unwrap_or_else(|e| panic!("{}: {e}", fixture.id));
            PREDICTION
                .check_slice(&fit.residuals, &duplicate(&fixture.residuals[rhs]))
                .unwrap_or_else(|e| panic!("{} residuals: {e}", fixture.id));
            PREDICTION
                .check_slice(&fit.fitted_values, &duplicate(&fixture.fitted[rhs]))
                .unwrap_or_else(|e| panic!("{} fitted: {e}", fixture.id));
            PREDICTION
                .check(fit.weighted_rss, 2.0 * fixture.objectives[rhs])
                .unwrap();
            let residuals = DenseMatrix::from_row_major(
                fixture.weights.len(),
                1,
                fixture.residuals[rhs].clone(),
            )
            .unwrap();
            let reference =
                DenseMatrix::from_row_major(fixture.weights.len(), 1, fixture.inputs[rhs].clone())
                    .unwrap();
            let audit = fe_remaining_projection_with_reference(
                &design,
                &fixture.weights,
                &residuals,
                &reference,
            )
            .unwrap();
            assert!(
                audit.maximum_relative_two_norm < 1e-9,
                "{}: {audit:?}",
                fixture.id
            );
        }
    }
}

#[test]
fn independent_projection_audit_detects_remaining_fe_content() {
    let design = FeDesign::new(
        4,
        vec![EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![]).unwrap()],
    )
    .unwrap();
    let residuals = DenseMatrix::from_row_major(4, 1, vec![1.0, 1.0, 2.0, 2.0]).unwrap();
    let audit =
        fe_remaining_projection_with_reference(&design, &[1.0; 4], &residuals, &residuals).unwrap();
    PREDICTION
        .check(audit.maximum_relative_two_norm, 1.0)
        .unwrap();
}

#[path = "support/engine.rs"]
mod engine_inputs;
#[test]
fn retained_lsmr_matches_hundred_digit_weak_graph_fixtures() {
    check_retained_fixtures(false);
}
#[test]
fn cmg_core_matches_hundred_digit_categorical_weak_graph_fixtures() {
    check_retained_fixtures(true);
}
fn check_retained_fixtures(cmg: bool) {
    use fereg_engine::{
        data::DenseMatrix as EngineMatrix,
        fe::{CmgCore, Design, LsmrSession, StopReason, Weights},
        threads::ThreadTeam,
    };
    let fixtures = parse_fixtures();
    assert_eq!(fixtures.len(), 6);
    for fixture in fixtures {
        let explicit = design(&fixture);
        let sample = engine_inputs::prepare(explicit.effects(), &fixture.inputs[0]);
        let design = Design::new(&sample, Weights::Provided(&fixture.weights)).unwrap();
        if cmg && explicit.effects().iter().any(|e| !e.slopes().is_empty()) {
            assert_eq!(
                CmgCore::new(&design, [0, 1]).err().unwrap().code,
                fereg_engine::ErrorCode::Unsupported
            );
            continue;
        }
        let rhs = EngineMatrix::from_columns(&fixture.inputs).unwrap();
        let mut reference = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let (mut coefficients, progress) = if cmg {
                let candidate = CmgCore::new(&design, [0, 1])
                    .unwrap()
                    .solve(&rhs, 16_000, 1e-12, &team)
                    .unwrap();
                (candidate.coefficients, candidate.progress)
            } else {
                let candidate = LsmrSession::new(&design, &team)
                    .unwrap_or_else(|e| panic!("{} build: {e}", fixture.id))
                    .solve(&rhs, 16_000, 1e-12, &team)
                    .unwrap_or_else(|e| panic!("{} solve: {e}", fixture.id));
                (candidate.coefficients, candidate.progress)
            };
            assert!(
                progress.iter().all(|p| p.stop == StopReason::Converged),
                "{}: {:?}",
                fixture.id,
                progress
            );
            design
                .normalize_coefficients(&mut coefficients, 1e-10, &team)
                .unwrap();
            let residuals = design.reconstruct(&rhs, &coefficients, &team).unwrap();
            let certificate = design
                .certify(&rhs, &coefficients, &residuals, 1e-10, &team)
                .unwrap_or_else(|e| panic!("{} certificate: {e}", fixture.id));
            for column in 0..rhs.ncols() {
                let residual = residuals.column(column).unwrap();
                let fitted: Vec<_> = fixture.inputs[column]
                    .iter()
                    .zip(&residual)
                    .map(|(y, r)| y - r)
                    .collect();
                PREDICTION
                    .check_slice(&residual, &fixture.residuals[column])
                    .unwrap_or_else(|e| panic!("{} residuals: {e}", fixture.id));
                PREDICTION
                    .check_slice(&fitted, &fixture.fitted[column])
                    .unwrap_or_else(|e| panic!("{} fitted: {e}", fixture.id));
                let objective: f64 = fixture
                    .weights
                    .iter()
                    .zip(&residual)
                    .map(|(w, r)| w * r * r)
                    .sum();
                PREDICTION
                    .check(objective, fixture.objectives[column])
                    .unwrap();
            }
            let actual = (coefficients, progress, residuals, certificate);
            if let Some(expected) = &reference {
                assert_eq!(expected, &actual, "{} workers={workers}", fixture.id);
            } else {
                reference = Some(actual);
            }
        }
    }
}
