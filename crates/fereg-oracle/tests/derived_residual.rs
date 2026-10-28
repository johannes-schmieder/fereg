//! Independent witnesses for the derived final residual (SPEC §7, §16) on the
//! direct one-FE route and the many-FE geometry policy: a Fast fit without
//! saved outputs keeps `ỹ - X̃b`, which must equal the full-dummy solution's
//! `y - Xb - D d̂` and whose explicit level moments must meet the requested
//! tolerance at the derived scale, at every worker count.
mod support;

use fereg_core::{
    CertifiedPreparedOlsFit, FeSolverKind, PreparedOlsExecutionMode, PreparedOlsFitConfig,
    SolveStopReason, ThreadConfig,
};
use fereg_oracle::compare::{COEFFICIENT, PREDICTION};
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_ols::{fit_explicit_ols, DummyEffect, OracleWeights};

const TOLERANCE: f64 = 1e-10;

fn design(codes: Vec<Vec<u32>>, weighted: bool) -> Design {
    let n = codes[0].len();
    let x: Vec<_> = (0..n)
        .map(|i| ((17 * i % 101) as f64 * 0.13).sin() + 0.002 * f64::from(codes[0][i]))
        .collect();
    let z: Vec<_> = (0..n)
        .map(|i| ((31 * i % 97) as f64 * 0.17).cos())
        .collect();
    let outcome = (0..n)
        .map(|i| {
            let effects: f64 = codes
                .iter()
                .enumerate()
                .map(|(j, c)| (f64::from(c[i]) * (0.7 + j as f64)).sin())
                .sum();
            1.2 * x[i] - 0.6 * z[i] + effects + 0.04 * ((i * 11 % 71) as f64).sin()
        })
        .collect();
    Design {
        outcome,
        regressors: vec![x, z],
        effects: codes
            .into_iter()
            .map(|codes| {
                let levels = codes.iter().copied().max().unwrap() as usize + 1;
                DummyEffect::new(codes, levels, true, vec![]).unwrap()
            })
            .collect(),
        weight_kind: if weighted {
            WeightKind::Analytic
        } else {
            WeightKind::Unit
        },
        weights: (0..n).map(|i| 0.5 + (i % 7) as f64 / 3.).collect(),
        missing: vec![false; n],
        clusters: vec![],
    }
}

/// One sorted FE of five rows per level, as in the campaign's one-FE design.
fn one_fe(weighted: bool) -> Design {
    design(vec![(0..1_000).map(|i| i / 5).collect()], weighted)
}

#[derive(Clone, Copy)]
enum Graph {
    /// Balanced cells: every pair is independent, so the MAP pilot certifies.
    Balanced,
    /// Few levels per FE with dependent pairs.
    Dense,
    /// A large sparse pair plus a small FE.
    Sparse,
}

fn three_fe(graph: Graph, weighted: bool) -> Design {
    let n = 1_200_u32;
    let codes = match graph {
        Graph::Balanced => vec![
            (0..n).map(|i| i % 4).collect(),
            (0..n).map(|i| i / 4 % 5).collect(),
            (0..n).map(|i| i / 20 % 3).collect(),
        ],
        Graph::Dense => vec![
            (0..n).map(|i| i % 12).collect(),
            (0..n).map(|i| (i * 7 + i / 12) % 9).collect(),
            (0..n).map(|i| (i * 5 + i / 30) % 7).collect(),
        ],
        Graph::Sparse => vec![
            (0..n).map(|i| i / 4).collect(),
            (0..n).map(|i| (i * 37 + i / 4 * 11) % 150).collect(),
            (0..n).map(|i| (i * 13 + i / 9) % 6).collect(),
        ],
    };
    design(codes, weighted)
}

fn config(workers: usize) -> PreparedOlsFitConfig {
    let threads = ThreadConfig::from_requested(workers, true).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.execution.mode = PreparedOlsExecutionMode::Fast;
    config.execution.threads = threads;
    config.absorb.lsmr.threads = threads;
    config.allow_final_fe_recovery = true;
    config
}

fn weighted_norm(values: &[f64], weights: &[f64]) -> f64 {
    values
        .iter()
        .zip(weights)
        .map(|(v, w)| w * v * v)
        .sum::<f64>()
        .sqrt()
}

/// Explicit dummy-level moments `|Σ_g w r| / sqrt(Σ_g w)` of every level of
/// every effect, relative to `s_y + Σ|b_j| s_j` from the raw columns (which
/// bound their projections' norms), as an independent §16 derived check.
fn derived_moment(design: &Design, residuals: &[f64], coefficients: &[f64]) -> f64 {
    let weights = match design.weight_kind {
        WeightKind::Unit => vec![1.; residuals.len()],
        _ => design.weights.clone(),
    };
    let scale = weighted_norm(&design.outcome, &weights)
        + design
            .regressors
            .iter()
            .zip(coefficients)
            .map(|(x, b)| b.abs() * weighted_norm(x, &weights))
            .sum::<f64>();
    let mut maximum = 0.0_f64;
    for effect in &design.effects {
        let mut sums = vec![(0.0, 0.0); effect.n_levels()];
        for (row, &code) in effect.codes().iter().enumerate() {
            sums[code as usize].0 += weights[row] * residuals[row];
            sums[code as usize].1 += weights[row];
        }
        for (moment, weight) in sums {
            maximum = maximum.max(moment.abs() / weight.sqrt());
        }
    }
    maximum / scale
}

fn check(design: &Design, workers: usize) -> CertifiedPreparedOlsFit {
    let (problem, expected_rows) = design.oracle_problem(false).unwrap();
    assert!(matches!(
        problem.weights,
        OracleWeights::Unit | OracleWeights::Wls(_)
    ));
    let oracle = fit_explicit_ols(&problem).expect("independent full-dummy OLS");
    let (fit, rows) = support::native_fit(design, config(workers), false).expect("prepared OLS");
    assert_eq!(rows, expected_rows);
    let report = fit.residual_absorption_report();
    assert_eq!(report.stop_reason, SolveStopReason::DerivedCertificate);
    assert_eq!((report.iterations, report.pilot_iterations), (0, 0));
    assert!(report.certified() && report.certificate.relative_infinity_norm <= TOLERANCE);
    assert_eq!(report.used_solver, fit.absorption_report().used_solver);
    let mut coefficients = Vec::new();
    for (actual, expected) in fit.coefficients().iter().zip(&oracle.coefficients) {
        let actual = actual.value.unwrap();
        COEFFICIENT.check(actual, expected.unwrap()).unwrap();
        coefficients.push(actual);
    }
    // The derived residual is the full solution's y - Xb - D d̂.
    PREDICTION
        .check_slice(fit.residuals(), &oracle.residuals)
        .unwrap();
    let moment = derived_moment(design, fit.residuals(), &coefficients);
    assert!(moment <= TOLERANCE, "derived moment {moment:.3e}");
    fit
}

#[test]
fn one_fe_derived_residual_matches_the_full_dummy_solution() {
    for weighted in [false, true] {
        let fit = check(&one_fe(weighted), 1);
        assert_eq!(fit.absorption_report().used_solver, FeSolverKind::Map);
    }
}

#[test]
fn many_fe_derived_residual_matches_the_full_dummy_solution() {
    let mut solvers = Vec::new();
    for graph in [Graph::Balanced, Graph::Dense, Graph::Sparse] {
        for weighted in [false, true] {
            let fit = check(&three_fe(graph, weighted), 1);
            solvers.push(fit.absorption_report().used_solver);
        }
    }
    // The MAP pilot needs unit weights and certifies the balanced graph; the
    // other designs run the LSMR session.
    assert_eq!(solvers[0], FeSolverKind::Map);
    assert!(
        solvers[1..].iter().all(|&s| s == FeSolverKind::Lsmr),
        "{solvers:?}"
    );
}

/// The derived row and the explicit derived moment of a fit too large for the
/// dense oracle; its residual must also match the polished final projection.
fn check_derived(design: &Design, workers: usize) -> CertifiedPreparedOlsFit {
    let (fit, _) = support::native_fit(design, config(workers), false).unwrap();
    let report = fit.residual_absorption_report();
    assert_eq!(report.stop_reason, SolveStopReason::DerivedCertificate);
    assert_eq!((report.iterations, report.pilot_iterations), (0, 0));
    let coefficients: Vec<_> = fit
        .coefficients()
        .iter()
        .map(|c| c.value.unwrap())
        .collect();
    let moment = derived_moment(design, fit.residuals(), &coefficients);
    assert!(moment <= TOLERANCE, "derived moment {moment:.3e}");
    fit
}

fn large(effects: usize, weighted: bool) -> Design {
    let n = 60_000_u32;
    let codes = if effects == 1 {
        vec![(0..n).map(|i| i / 5).collect()]
    } else {
        vec![
            (0..n).map(|i| i / 5).collect(),
            (0..n).map(|i| (i * 37 + i / 5 * 11) % 2_000).collect(),
            (0..n).map(|i| (i * 13 + i / 9) % 25).collect(),
        ]
    };
    design(codes, weighted)
}

/// Same worker count repeats bit for bit; worker counts agree scientifically
/// and in every discrete decision, above the parallel reduction floors.
#[test]
fn derived_residual_repeats_and_agrees_across_workers() {
    for design in [large(1, false), large(3, false), large(3, true)] {
        let mut polished_config = config(4);
        polished_config.final_residual_tolerance = Some(1e-14);
        let (polished, _) = support::native_fit(&design, polished_config, false).unwrap();
        assert_eq!(
            polished.residual_absorption_report().stop_reason,
            SolveStopReason::Certified
        );
        let reference = check_derived(&design, 1);
        for workers in [1, 2, 4, 8] {
            let fit = check_derived(&design, workers);
            let (repeat, _) = support::native_fit(&design, config(workers), false).unwrap();
            let bits = |fit: &CertifiedPreparedOlsFit| {
                fit.residuals()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>()
            };
            assert_eq!(bits(&fit), bits(&repeat), "workers={workers}");
            for (a, b) in fit.coefficients().iter().zip(repeat.coefficients()) {
                assert_eq!(a.value.map(f64::to_bits), b.value.map(f64::to_bits));
            }
            let pairs = [
                (fit.absorption_report(), reference.absorption_report()),
                (
                    fit.residual_absorption_report(),
                    reference.residual_absorption_report(),
                ),
            ];
            for (report, other) in pairs {
                assert_eq!(report.used_solver, other.used_solver);
                assert_eq!(report.used_preconditioner, other.used_preconditioner);
                assert_eq!(report.stop_reason, other.stop_reason);
                assert_eq!(report.fallback_used, other.fallback_used);
            }
            PREDICTION
                .check_slice(fit.residuals(), reference.residuals())
                .unwrap();
            PREDICTION
                .check_slice(fit.residuals(), polished.residuals())
                .unwrap();
        }
    }
}
