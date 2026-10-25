#![deny(clippy::too_many_lines)]
//! Independent H2 witnesses through the public prepared-OLS fit entry point.
//! Explicit diagonal preconditioning retains the geometry-led MAP policy;
//! Auto/Auto routing has separate strict-policy witnesses in routing_portfolio.
mod support;

use fereg_core::fe::LsmrPreconditioner;
use fereg_core::{
    CertifiedPreparedOlsFit, FeSolverKind, PreparedOlsExecutionMode, PreparedOlsFitConfig,
    ThreadConfig,
};
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_dummy::{
    design::ExplicitFeDesign, fe_remaining_projection_with_reference,
};
use fereg_oracle::explicit_ols::{fit_explicit_ols, DummyEffect, OracleMatrix};

/// Unequal cell counts factor into three marginal multiplicities. Appending
/// one observation perturbs independence without changing any level or rank.
fn product_design(perturbed: bool) -> Design {
    let mut codes: [Vec<u32>; 3] = std::array::from_fn(|_| Vec::new());
    for (a, na) in [1, 2, 3].into_iter().enumerate() {
        for (b, nb) in [1, 2, 4].into_iter().enumerate() {
            for (c, nc) in [1, 3].into_iter().enumerate() {
                for _ in 0..8 * na * nb * nc {
                    codes[0].push(a as u32);
                    codes[1].push(b as u32);
                    codes[2].push(c as u32);
                }
            }
        }
    }
    if perturbed {
        for code in &mut codes {
            code.push(0);
        }
    }
    from_codes(codes)
}

fn from_codes(codes: [Vec<u32>; 3]) -> Design {
    let n = codes[0].len();
    let x: Vec<_> = (0..n)
        .map(|i| ((17 * i % 101) as f64 * 0.13).sin())
        .collect();
    let z: Vec<_> = (0..n)
        .map(|i| ((31 * i % 97) as f64 * 0.17).cos())
        .collect();
    let outcome = (0..n)
        .map(|i| {
            1.2 * x[i] - 0.6 * z[i] + 0.4 * f64::from(codes[0][i]) - 0.3 * f64::from(codes[1][i])
                + 0.2 * f64::from(codes[2][i])
                + 0.04 * ((i * 11 % 71) as f64).sin()
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
        weight_kind: WeightKind::Unit,
        weights: vec![1.; n],
        missing: vec![false; n],
        clusters: vec![],
    }
}

/// Independent explicit contingency rectangle, not the production streaming
/// cancellation identity. This deliberately retains extra common directions.
fn pair_bound(design: &Design, i: usize, j: usize) -> f64 {
    let a = &design.effects[i];
    let b = &design.effects[j];
    let n = design.outcome.len() as i128;
    let mut rows = vec![0_i128; a.n_levels()];
    let mut columns = vec![0_i128; b.n_levels()];
    let mut counts = vec![vec![0_i128; b.n_levels()]; a.n_levels()];
    for (&a, &b) in a.codes().iter().zip(b.codes()) {
        rows[a as usize] += 1;
        columns[b as usize] += 1;
        counts[a as usize][b as usize] += 1;
    }
    let mut numerator = 0_i128;
    for (r, &nr) in rows.iter().enumerate() {
        for (c, &nc) in columns.iter().enumerate() {
            numerator += (n * counts[r][c] - nr * nc).pow(2);
        }
    }
    let denominator = n * n * rows.iter().min().unwrap() * columns.iter().min().unwrap();
    (numerator as f64 / denominator as f64).sqrt().min(1.)
}

fn config(mode: PreparedOlsExecutionMode, workers: usize, retain: bool) -> PreparedOlsFitConfig {
    let threads = ThreadConfig::from_requested(workers, true).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.execution.mode = mode;
    config.execution.poolsize = 1;
    config.execution.threads = threads;
    config.absorb.lsmr.threads = threads;
    config.absorb.lsmr.preconditioner = LsmrPreconditioner::Diagonal;
    config.absorb.retain_coefficients = retain;
    config.allow_final_fe_recovery = true;
    config.final_residual_tolerance = Some(1e-14);
    config
}

fn check_oracle(design: &Design, config: PreparedOlsFitConfig) -> CertifiedPreparedOlsFit {
    let (problem, expected_rows) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).expect("independent full-dummy OLS");
    let (fit, rows) = support::native_fit(design, config, false).expect("prepared OLS");
    assert_eq!(rows, expected_rows);
    assert_eq!(rows.len(), design.outcome.len());
    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank);
    assert!(fit.dropped_generated_columns().is_empty());
    for (actual, expected) in fit.coefficients().iter().zip(&oracle.coefficients) {
        COEFFICIENT
            .check(actual.value.unwrap(), expected.unwrap())
            .unwrap();
    }
    PREDICTION
        .check_slice(fit.residuals(), &oracle.residuals)
        .unwrap();
    let fitted: Vec<_> = design
        .outcome
        .iter()
        .zip(fit.residuals())
        .map(|(y, r)| y - r)
        .collect();
    PREDICTION
        .check_slice(&fitted, &oracle.fitted_values)
        .unwrap();
    let mut projection = problem.clone();
    projection.regressors.clear();
    let within = fit_explicit_ols(&projection).expect("independent outcome FE projection");
    INFERENCE
        .check(
            fit.initial_absorbed_outcome_sum_of_squares(),
            within.weighted_rss,
        )
        .unwrap();
    assert!(fit.absorption_report().certified());
    assert!(fit.residual_absorption_report().certified());
    // Final residuals may also take the bounded pilot; whichever solver
    // finishes, the independent certificate accepts at the 1e-14 target.
    let residual = fit.residual_absorption_report();
    match residual.used_solver {
        FeSolverKind::Map => assert!(residual.pilot_iterations <= 4, "{residual:?}"),
        solver => assert_eq!(solver, FeSolverKind::Lsmr),
    }
    assert!(
        residual.certificate.relative_infinity_norm <= 1e-14,
        "{residual:?}"
    );
    if config.absorb.retain_coefficients && config.execution.mode == PreparedOlsExecutionMode::Fast
    {
        assert!(fit.recovered_fe_coefficients().is_some());
    }
    let explicit = ExplicitFeDesign::new(rows.len(), problem.effects).unwrap();
    let weights = match problem.weights {
        fereg_oracle::explicit_ols::OracleWeights::Unit => vec![1.; rows.len()],
        fereg_oracle::explicit_ols::OracleWeights::Wls(w) => w,
        fereg_oracle::explicit_ols::OracleWeights::Frequency(_) => unreachable!(),
    };
    let residuals = OracleMatrix::from_row_major(rows.len(), 1, fit.residuals().to_vec()).unwrap();
    let original = OracleMatrix::from_row_major(rows.len(), 1, problem.outcome).unwrap();
    let audit =
        fe_remaining_projection_with_reference(&explicit, &weights, &residuals, &original).unwrap();
    assert!(audit.maximum_relative_two_norm < 1e-10, "{audit:?}");
    fit
}

#[test]
fn nonuniform_independent_and_perturbed_marginals_select_certified_map() {
    for perturbed in [false, true] {
        let design = product_design(perturbed);
        for (i, j) in [(0, 1), (0, 2), (1, 2)] {
            let bound = pair_bound(&design, i, j);
            assert!(bound < 0.01);
            if !perturbed {
                assert_eq!(bound, 0.);
            }
        }
        // For every a,b,c <= .01, the positive 2x2 majorant has maximum
        // row sum < .001. Four powers are therefore <1e-12, safely below
        // the ordinary 1e-12 recurrence target, without reproducing it.
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for workers in [1, 4] {
                for retain in [false, true] {
                    let fit = check_oracle(&design, config(mode, workers, retain));
                    assert_eq!(fit.absorption_report().used_solver, FeSolverKind::Map);
                    assert!(!fit.absorption_report().fallback_used);
                }
            }
        }
    }
}

#[test]
fn extra_pair_intersection_and_disconnected_components_stay_on_lsmr() {
    for disconnected in [false, true] {
        let mut codes: [Vec<u32>; 3] = std::array::from_fn(|_| Vec::new());
        for component in 0..2 {
            for a in 0..2 {
                for b in 0..2 {
                    for c in 0..2 {
                        for _ in 0..8 {
                            codes[0].push(component * 2 + a);
                            codes[1].push(component * 2 + b);
                            codes[2].push(if disconnected { component * 2 + c } else { c });
                        }
                    }
                }
            }
        }
        let design = from_codes(codes);
        assert_eq!(pair_bound(&design, 0, 1), 1.);
        // When connected through effect 3, the common direction of effects
        // 1/2 is still real and must not be removed by pairwise centering.
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            let fit = check_oracle(&design, config(mode, 1, true));
            assert_eq!(fit.absorption_report().used_solver, FeSolverKind::Lsmr);
        }
    }
}

#[test]
fn changing_observation_weights_do_not_use_unit_geometry() {
    let mut design = product_design(false);
    design.weight_kind = WeightKind::Analytic;
    design.weights = (0..design.outcome.len())
        .map(|i| 0.5 + (i % 7) as f64 / 5.)
        .collect();
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let fit = check_oracle(&design, config(mode, 1, true));
        assert_eq!(fit.absorption_report().used_solver, FeSolverKind::Lsmr);
    }
}

#[test]
fn explicit_lsmr_preserves_its_route_on_map_eligible_geometry() {
    let design = product_design(true);
    let mut config = config(PreparedOlsExecutionMode::Fast, 1, true);
    config.absorb.solver = FeSolverKind::Lsmr;
    let fit = check_oracle(&design, config);
    assert_eq!(fit.absorption_report().used_solver, FeSolverKind::Lsmr);
}

#[test]
fn fe_only_projection_matches_full_dummy_qr() {
    let mut design = product_design(true);
    design.regressors.clear();
    let fit = check_oracle(&design, config(PreparedOlsExecutionMode::Fast, 1, true));
    assert_eq!(fit.absorption_report().used_solver, FeSolverKind::Map);
}

#[test]
fn parallel_eligible_projection_matches_dummy_qr_and_repeats_exactly() {
    let small = product_design(false);
    let codes: [Vec<u32>; 3] = std::array::from_fn(|e| small.effects[e].codes().repeat(16));
    let design = from_codes(codes);
    assert!(design.outcome.len() >= 16_384);
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        let mut expected_iterations = None;
        for workers in [1, 4, 16] {
            let config = config(mode, workers, true);
            let first = check_oracle(&design, config);
            let (again, _) = support::native_fit(&design, config, false).unwrap();
            assert_eq!(first.absorption_report().used_solver, FeSolverKind::Map);
            let iterations = first.absorption_report().iterations;
            assert_eq!(iterations, *expected_iterations.get_or_insert(iterations));
            assert_eq!(iterations, again.absorption_report().iterations);
            assert_eq!(first.coefficients(), again.coefficients());
            assert_eq!(first.residuals(), again.residuals());
        }
    }
}
