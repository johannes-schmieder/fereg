#![deny(clippy::too_many_lines)]
//! Absorbed varying slopes on every supported OLS route against the
//! independent explicit-design QR, including singular slope levels (a slope
//! constant or zero on a level, a single-row level), and saved-effect
//! recovery: fedemean's full system and retained coefficients against the
//! established MAP recovery, the SPEC §5 gauge and fereg's representative on
//! singular levels.
mod support;

use fereg_core::fe::{
    recover_finalized_categorical_effects_map_certified, CategoricalAbsorbedEffectView,
    LsmrPreconditioner,
};
use fereg_core::{
    fit_prepared_model_coefficients, prepare_model, BorrowedMapWeights, CertifiedPreparedOlsFit,
    FeSolverKind, MapConfig, PreparedOlsData, PreparedOlsExecutionMode, PreparedOlsFitConfig,
    ThreadConfig,
};
use fereg_oracle::compare::{Tolerance, COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_ols::{fit_explicit_ols, DummyEffect, OracleWeights};

/// SPEC §20's saved-output envelope.
const SAVED: Tolerance = Tolerance {
    relative: 1e-7,
    absolute: 1e-9,
};

/// Which effect carries the slope in a [`panel`].
#[derive(Clone, Copy, Debug)]
enum Shape {
    /// `worker firm##c.s`.
    FirmSlope,
    /// `worker##c.t firm`.
    WorkerSlope,
    /// `worker firm##c.s year`.
    ThreeWay,
    /// `worker year firm#c.s` (slope only).
    SlopeOnly,
}

/// A connected worker-firm-year panel: 48 workers over six years with
/// movers. The firm slope `s` is constant on firm 0, zero on firm 1, and
/// firm 7 has a single row (all singular firm channel blocks).
fn panel(shape: Shape, weighted: bool) -> Design {
    let (workers, years) = (48, 6);
    let n = workers * years;
    let mut worker = Vec::with_capacity(n);
    let mut firm = Vec::with_capacity(n);
    let mut year = Vec::with_capacity(n);
    for w in 0..workers {
        for t in 0..years {
            worker.push(w as u32);
            year.push(t as u32);
            // Firms 0..7 by mobility; row 0 alone in firm 7.
            let f = if w == 0 && t == 0 {
                7
            } else {
                ((w + t / 2) % 7) as u32
            };
            firm.push(f);
        }
    }
    let t: Vec<f64> = year.iter().map(|&y| 1.0 + f64::from(y)).collect();
    let s: Vec<f64> = (0..n)
        .map(|i| match firm[i] {
            0 => 2.5,
            1 => 0.0,
            _ => t[i] + 0.25 * ((i * 7 % 11) as f64 - 5.0) / 5.0,
        })
        .collect();
    let x1: Vec<f64> = (0..n).map(|i| ((i * 13 % 17) as f64 - 8.0) / 4.0).collect();
    let x2: Vec<f64> = (0..n)
        .map(|i| (i as f64 * 0.37).sin() + 0.1 * t[i])
        .collect();
    let outcome = (0..n)
        .map(|i| {
            0.5 * x1[i] - 0.25 * x2[i]
                + f64::from(worker[i] % 5) * 0.2
                + f64::from(firm[i]) * 0.1 * s[i]
                + f64::from(firm[i] % 3) * 0.3
                + 0.05 * f64::from(year[i])
                + ((i * 31 % 23) as f64 - 11.0) / 50.0
        })
        .collect();
    let intercept =
        |codes: &Vec<u32>, levels| DummyEffect::new(codes.clone(), levels, true, vec![]);
    let effects = match shape {
        Shape::FirmSlope => vec![
            intercept(&worker, workers),
            DummyEffect::new(firm.clone(), 8, true, vec![s]),
        ],
        Shape::WorkerSlope => vec![
            DummyEffect::new(worker.clone(), workers, true, vec![t]),
            intercept(&firm, 8),
        ],
        Shape::ThreeWay => vec![
            intercept(&worker, workers),
            DummyEffect::new(firm.clone(), 8, true, vec![s]),
            intercept(&year, years),
        ],
        Shape::SlopeOnly => vec![
            intercept(&worker, workers),
            intercept(&year, years),
            DummyEffect::new(firm.clone(), 8, false, vec![s]),
        ],
    }
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
    Design {
        outcome,
        regressors: vec![x1, x2],
        effects,
        weights: (0..n)
            .map(|i| {
                if weighted {
                    1.0 + (i % 5) as f64 / 3.0
                } else {
                    1.0
                }
            })
            .collect(),
        weight_kind: if weighted {
            WeightKind::Analytic
        } else {
            WeightKind::Unit
        },
        missing: vec![false; n],
        clusters: vec![],
    }
}

fn config(
    solver: FeSolverKind,
    preconditioner: LsmrPreconditioner,
    workers: usize,
) -> PreparedOlsFitConfig {
    let threads = ThreadConfig::from_requested(workers, true).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.execution.threads = threads;
    config.absorb.lsmr.threads = threads;
    config.absorb.solver = solver;
    config.absorb.lsmr.preconditioner = preconditioner;
    config.absorb.retain_coefficients = true;
    // Pointwise residual comparisons need a projection tighter than the
    // default moment tolerance (any preconditioner, including none).
    config.absorb.map.tolerance = 1e-10;
    config.absorb.lsmr.tolerance = 1e-10;
    config
}

fn prepared(design: &Design) -> PreparedOlsData {
    let (request, loaded) = support::inputs(design, true);
    prepare_model(&request, &[], &loaded, true).unwrap()
}

/// The fit against the explicit-design QR: residuals, fitted values,
/// coefficients, rank and the weighted RSS.
fn check_against_oracle(design: &Design, fit: &CertifiedPreparedOlsFit, context: &str) {
    let (problem, rows) = design.oracle_problem(true).unwrap();
    assert_eq!(rows, (0..design.outcome.len()).collect::<Vec<_>>());
    let oracle = fit_explicit_ols(&problem).unwrap();
    PREDICTION
        .check_slice(fit.residuals(), &oracle.residuals)
        .unwrap_or_else(|e| panic!("residuals {context}: {e}"));
    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank, "{context}");
    for (actual, expected) in fit.coefficients().iter().zip(&oracle.coefficients) {
        COEFFICIENT
            .check(actual.value.unwrap(), expected.unwrap())
            .unwrap_or_else(|e| panic!("coefficient {context}: {e}"));
    }
    let q = match problem.weights {
        OracleWeights::Unit => vec![1.0; rows.len()],
        OracleWeights::Wls(w) => w,
        OracleWeights::Frequency(w) => w.into_iter().map(f64::from).collect(),
    };
    let rss = fit
        .residuals()
        .iter()
        .zip(&q)
        .map(|(&r, w)| r * r * w)
        .sum::<f64>();
    INFERENCE
        .check(rss, oracle.weighted_rss)
        .unwrap_or_else(|e| panic!("RSS {context}: {e}"));
}

#[test]
fn slope_designs_match_the_explicit_design_on_every_route() {
    let lsmr = [
        LsmrPreconditioner::Auto,
        LsmrPreconditioner::Off,
        LsmrPreconditioner::Diagonal,
        LsmrPreconditioner::Schwarz,
    ];
    for shape in [
        Shape::FirmSlope,
        Shape::WorkerSlope,
        Shape::ThreeWay,
        Shape::SlopeOnly,
    ] {
        for weighted in [false, true] {
            let design = panel(shape, weighted);
            let prepared = prepared(&design);
            let routes = std::iter::once((FeSolverKind::Auto, LsmrPreconditioner::Auto))
                .chain(lsmr.iter().map(|&p| (FeSolverKind::Lsmr, p)))
                .chain(std::iter::once((
                    FeSolverKind::Map,
                    LsmrPreconditioner::Auto,
                )));
            for (solver, preconditioner) in routes {
                let mut fits = Vec::new();
                for workers in [1, 4, 4] {
                    let context = format!("{shape:?} weighted={weighted} {solver:?} {preconditioner:?} workers={workers}");
                    let fit = fit_prepared_model_coefficients(
                        &prepared,
                        config(solver, preconditioner, workers),
                    )
                    .unwrap_or_else(|e| panic!("{context}: {e}"));
                    assert!(fit.absorption_report().certified(), "{context}");
                    check_against_oracle(&design, &fit, &context);
                    fits.push(fit);
                }
                // Repeatable at a fixed worker count.
                assert_eq!(fits[1].residuals(), fits[2].residuals());
                assert_eq!(fits[1].coefficients(), fits[2].coefficients());
                if solver == FeSolverKind::Auto {
                    let mut low = config(solver, preconditioner, 4);
                    low.execution.mode = PreparedOlsExecutionMode::Low;
                    low.execution.poolsize = 1;
                    let fit = fit_prepared_model_coefficients(&prepared, low).unwrap();
                    check_against_oracle(&design, &fit, &format!("{shape:?} {weighted} low"));
                }
            }
        }
    }
}

#[test]
fn auto_runs_fedemean_full_system_on_many_effect_slope_designs() {
    for shape in [Shape::ThreeWay, Shape::SlopeOnly] {
        let design = panel(shape, false);
        let (fit, _) = support::native_fit(
            &design,
            config(FeSolverKind::Auto, LsmrPreconditioner::Auto, 4),
            true,
        )
        .unwrap();
        let report = fit.absorption_report();
        assert_eq!(report.used_solver, FeSolverKind::Cg, "{shape:?}");
        assert_eq!(
            report.route().reduction,
            fereg_core::FeReduction::Full,
            "{shape:?}"
        );
        assert!(!report.fallback_used, "{shape:?}");
        // fedemean's channel-major coefficients are retained.
        assert!(fit.recovered_fe_coefficients().is_some(), "{shape:?}");
    }
}

/// The absorbed target `y - xb - e` of a fit.
fn target(prepared: &PreparedOlsData, design: &Design, fit: &CertifiedPreparedOlsFit) -> Vec<f64> {
    (0..design.outcome.len())
        .map(|row| {
            let xb: f64 = fit
                .coefficients()
                .iter()
                .filter_map(|c| {
                    Some(c.value? * prepared.column_value(c.generated_column_index?, row).ok()?)
                })
                .sum();
            prepared.sample().outcome[row] - xb - fit.residuals()[row]
        })
        .collect()
}

#[test]
fn saved_slope_effects_match_map_recovery_with_the_spec_gauge_and_singular_rule() {
    for shape in [Shape::FirmSlope, Shape::ThreeWay, Shape::SlopeOnly] {
        let design = panel(shape, false);
        let prepared = prepared(&design);
        let fit = fit_prepared_model_coefficients(
            &prepared,
            config(FeSolverKind::Auto, LsmrPreconditioner::Auto, 4),
        )
        .unwrap();
        let target = target(&prepared, &design, &fit);
        let channels: usize = design
            .effects
            .iter()
            .map(|e| usize::from(e.has_intercept()) + e.slopes().len())
            .sum();
        let requested = vec![true; channels];
        let map = MapConfig {
            tolerance: 1e-12,
            max_iterations: 20_000,
            check_every: 1,
            symmetric: true,
        };
        let recover = |retained| {
            prepared
                .recover_absorbed_effects_fast(&target, &requested, retained, true, true, map, None)
                .unwrap()
        };
        let solved = recover(None);
        let retained = recover(fit.recovered_fe_coefficients());
        let views: Vec<_> = design
            .effects
            .iter()
            .map(|e| {
                CategoricalAbsorbedEffectView::new(
                    e.has_intercept(),
                    e.slopes().iter().map(Vec::as_slice).collect(),
                )
                .unwrap()
            })
            .collect();
        let reference = recover_finalized_categorical_effects_map_certified(
            prepared.fe_topology(),
            &views,
            BorrowedMapWeights::Unit,
            &target,
            &requested,
            map,
        )
        .unwrap();
        let reference = spec_gauge(&design, reference.channels());
        for (label, recovered) in [("full system", &solved), ("retained", &retained)] {
            PREDICTION
                .check_slice(recovered.absorbed_sum(), &target)
                .unwrap();
            for (index, (actual, expected)) in
                recovered.channels().iter().zip(&reference).enumerate()
            {
                SAVED
                    .check_slice(actual.as_ref().unwrap(), expected)
                    .unwrap_or_else(|e| panic!("{shape:?} {label} channel {index}: {e}"));
            }
            singular_rule(&design, recovered.channels());
        }
    }
}

/// The established MAP decomposition in SPEC §5's gauge: on this connected
/// design, every intercept effect after the first is centered (unit
/// weights) and its mean moves to the first intercept effect.
fn spec_gauge(design: &Design, channels: &[Option<Vec<f64>>]) -> Vec<Vec<f64>> {
    let mut channels: Vec<Vec<f64>> = channels.iter().map(|c| c.clone().unwrap()).collect();
    let mut flat = 0;
    let mut intercepts = Vec::new();
    for effect in &design.effects {
        if effect.has_intercept() {
            intercepts.push(flat);
        }
        flat += usize::from(effect.has_intercept()) + effect.slopes().len();
    }
    let n = design.outcome.len() as f64;
    for &channel in intercepts.iter().skip(1) {
        let mean = channels[channel].iter().sum::<f64>() / n;
        channels[channel].iter_mut().for_each(|v| *v -= mean);
        channels[intercepts[0]].iter_mut().for_each(|v| *v += mean);
    }
    channels
}

/// fereg's representative on the singular firm levels: the slope channel is
/// exactly zero where `s` is constant (firm 0), zero (firm 1) or observed
/// once (firm 7), which keep the intercept.
fn singular_rule(design: &Design, channels: &[Option<Vec<f64>>]) {
    let mut flat = 0;
    for effect in &design.effects {
        if !effect.slopes().is_empty() {
            let slope = flat + usize::from(effect.has_intercept());
            let values = channels[slope].as_ref().unwrap();
            for (row, &code) in effect.codes().iter().enumerate() {
                if effect.has_intercept() && matches!(code, 0 | 1 | 7) {
                    assert_eq!(values[row], 0.0, "slope channel on singular level {code}");
                }
            }
        }
        flat += usize::from(effect.has_intercept()) + effect.slopes().len();
    }
}

/// `worker firm##c.(s1-s16) year`: 17 channels on one effect (beyond the
/// former 16-channel limit) certify on fedemean's routes against the explicit
/// design (Auto: full x cg, explicit LSMR), and saved effects recover.
#[test]
fn seventeen_channels_run_on_fedemean_against_the_explicit_design() {
    let base = panel(Shape::ThreeWay, false);
    let n = base.outcome.len();
    let slopes: Vec<Vec<f64>> = (0..16)
        .map(|s| {
            (0..n)
                .map(|i| {
                    ((i * (2 * s + 3) + s) % 29) as f64 / 7.0 + (i as f64 * 0.3 + s as f64).sin()
                })
                .collect()
        })
        .collect();
    let mut design = base.clone();
    let firm = design.effects[1].codes().to_vec();
    // Every firm level needs more distinct rows than channels: firm 7's
    // single row joins firm 6 here.
    let firm: Vec<u32> = firm.iter().map(|&f| f.min(6)).collect();
    design.effects[1] = DummyEffect::new(firm, 7, true, slopes).unwrap();
    let prepared = prepared(&design);
    for (solver, used) in [
        (FeSolverKind::Auto, FeSolverKind::Cg),
        (FeSolverKind::Lsmr, FeSolverKind::Lsmr),
    ] {
        let fit =
            fit_prepared_model_coefficients(&prepared, config(solver, LsmrPreconditioner::Auto, 4))
                .unwrap_or_else(|e| panic!("{solver:?}: {e}"));
        let report = fit.absorption_report();
        assert!(report.certified(), "{solver:?}");
        assert!(!report.fallback_used, "{solver:?}");
        assert_eq!(report.used_solver, used, "{solver:?}");
        check_against_oracle(&design, &fit, &format!("17 channels {solver:?}"));
        if solver == FeSolverKind::Auto {
            let target = target(&prepared, &design, &fit);
            let channels = 1 + 17 + 1;
            let saved = prepared
                .recover_absorbed_effects_fast(
                    &target,
                    &vec![true; channels],
                    fit.recovered_fe_coefficients(),
                    true,
                    true,
                    MapConfig {
                        tolerance: 1e-12,
                        max_iterations: 20_000,
                        check_every: 1,
                        symmetric: true,
                    },
                    None,
                )
                .unwrap();
            PREDICTION
                .check_slice(saved.absorbed_sum(), &target)
                .unwrap();
        }
    }
}

/// Explicit `solver(cg, reduce() precondition())` on slope designs runs
/// fedemean's PCG with the requested reduction and preconditioner against
/// the explicit-design QR: the full system with none, Jacobi (each level's
/// channel blocks) or Schwarz (pair subdomains), and the elimination for two
/// effects. A choice fedemean does not take on the design (the elimination
/// of three effects, block-Jacobi with slopes) continues on Auto's certified
/// full-system PCG, reported as the fallback with the reason `unsupported`.
#[test]
fn explicit_cg_on_slope_designs_matches_the_explicit_design() {
    use fereg_core::{
        FeFallbackReason, FeMethod, FePreconditioner as Used, FeReduction, PcgPreconditioner as P,
        PcgReduction as R, PcgRequest,
    };
    let requests = [
        (R::Auto, P::Auto),
        (R::Auto, P::Jacobi),
        (R::Auto, P::None),
        (R::Auto, P::Schwarz),
        (R::Auto, P::Block),
        (R::Full, P::Auto),
        (R::Full, P::None),
        (R::Full, P::Jacobi),
        (R::Full, P::Schwarz),
        (R::Eliminate, P::Auto),
        (R::Eliminate, P::Jacobi),
        (R::Eliminate, P::Block),
    ];
    for shape in [
        Shape::FirmSlope,
        Shape::WorkerSlope,
        Shape::ThreeWay,
        Shape::SlopeOnly,
    ] {
        for weighted in [false, true] {
            let design = panel(shape, weighted);
            let prepared = prepared(&design);
            let effects = design.effects.len();
            for (reduction, preconditioner) in requests {
                let context =
                    format!("{shape:?} weighted={weighted} {reduction:?} {preconditioner:?}");
                let mut config = config(FeSolverKind::Pcg, LsmrPreconditioner::Auto, 4);
                config.absorb.pcg = PcgRequest {
                    reduction,
                    preconditioner,
                };
                let fit = fit_prepared_model_coefficients(&prepared, config)
                    .unwrap_or_else(|e| panic!("{context}: {e}"));
                let report = fit.absorption_report();
                assert!(report.certified(), "{context}");
                assert_eq!(report.requested_solver, FeSolverKind::Pcg, "{context}");
                check_against_oracle(&design, &fit, &context);
                let route = report.route();
                assert_eq!(route.method, FeMethod::Cg, "{context}");
                let unsupported =
                    preconditioner == P::Block || (reduction == R::Eliminate && effects != 2);
                assert_eq!(report.fallback_used, unsupported, "{context}");
                if unsupported {
                    assert_eq!(
                        report.fallback_reason,
                        FeFallbackReason::Unsupported,
                        "{context}"
                    );
                    assert_eq!(route.reduction, FeReduction::Full, "{context}");
                    continue;
                }
                let eliminated = reduction == R::Eliminate
                    || (reduction == R::Auto
                        && effects == 2
                        && matches!(preconditioner, P::Auto | P::Jacobi));
                let expected = if eliminated {
                    FeReduction::Eliminate
                } else {
                    FeReduction::Full
                };
                assert_eq!(route.reduction, expected, "{context}");
                match preconditioner {
                    P::None => assert_eq!(route.preconditioner, Used::None, "{context}"),
                    P::Jacobi => assert_eq!(route.preconditioner, Used::Jacobi, "{context}"),
                    P::Schwarz => assert!(
                        matches!(route.preconditioner, Used::Schwarz(_)),
                        "{context}"
                    ),
                    _ => {}
                }
            }
        }
    }
}
