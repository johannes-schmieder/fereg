#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
//! M3.6 worker. Loading/preparation and finite fitting are outside FE stage timing.
use fereg_engine::{
    data::DenseMatrix,
    fe::{Design, Engine, Weights},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::{self, Policy, Problem, Role, Slot},
};
use fereg_oracle::designs::Design as Input;
use std::{error::Error, path::Path, time::Instant};
#[path = "../tests/support/engine.rs"]
mod engine_inputs;
#[path = "fe_projection/io.rs"]
mod io;
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod legacy_inputs;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: fe_projection INPUT ARM THREADS TOLERANCE OUTPUT_DIRECTORY".into());
    }
    let input = io::read(Path::new(&args[0]))?;
    let workers = args[2].parse::<usize>()?;
    let tolerance = args[3].parse::<f64>()?;
    if workers == 0 || !tolerance.is_finite() || tolerance <= 0. || tolerance >= 1. {
        return Err("invalid benchmark controls".into());
    }
    let destination = Path::new(&args[4]);
    match args[1].as_str() {
        "legacy-auto" => legacy(&input, workers, tolerance, destination),
        "next-auto" => next(&input, workers, tolerance, Solver::Auto, destination),
        "next-lsmr" => next(&input, workers, tolerance, Solver::Lsmr, destination),
        "next-cmg" => next(&input, workers, tolerance, Solver::Cmg, destination),
        _ => Err("unknown benchmark arm".into()),
    }
}

fn legacy(input: &Input, workers: usize, tolerance: f64, destination: &Path) -> Result<()> {
    use fereg_core::{
        fit_prepared_categorical_ols_coefficients_map, prepare_categorical_ols_data,
        PreparedOlsFitConfig, PreparedOlsFitStatus, ThreadConfig,
    };
    if std::env::var_os("FEREG_PHASE_TIMING").is_none() {
        return Err("legacy benchmark requires FEREG_PHASE_TIMING".into());
    }
    let (request, loaded) = legacy_inputs::inputs(input, true);
    let prepared = prepare_categorical_ols_data(&request, &loaded)?;
    drop((request, loaded));
    assert_eq!(prepared.sample().n_active(), input.outcome.len());
    assert_eq!(prepared.model().columns.len(), input.regressors.len() + 1);
    let mut config = PreparedOlsFitConfig {
        allow_final_fe_recovery: true,
        ..PreparedOlsFitConfig::default()
    };
    config.absorb.map.tolerance = tolerance;
    config.absorb.map.max_iterations = 16_000;
    config.absorb.lsmr.tolerance = tolerance;
    config.absorb.lsmr.max_iterations = 16_000;
    config.absorb.lsmr.threads = ThreadConfig::from_requested(workers, true)?;
    config.execution.threads = config.absorb.lsmr.threads;
    // The IID estimator calls this prepared entry point. Its existing phase
    // trace includes numerical setup in `rhs`, then raw moments and absorption;
    // lower-level legacy FE APIs have different automatic routing.
    let fit = fit_prepared_categorical_ols_coefficients_map(&prepared, config)?;
    let report = fit.absorption_report();
    if !report.certified() || fit.finite_rank() != input.regressors.len() {
        return Err("legacy benchmark projection/rank did not certify".into());
    }
    let mut coefficients = Vec::new();
    for coefficient in fit.coefficients().iter().take(input.regressors.len()) {
        if coefficient.status != PreparedOlsFitStatus::Kept {
            return Err("legacy benchmark ordinary coefficient was omitted".into());
        }
        coefficients.push(coefficient.value.ok_or("missing legacy coefficient")?);
    }
    io::write_values(&destination.join("coefficients.f64"), &coefficients)?;
    io::write_values(&destination.join("residuals.f64"), fit.residuals())?;
    let metadata = format!(
        "rows\t{}\nrhs_count\t{}\nrank\t{}\nused\t{:?}\niterations\t{}\nfallbacks\t{}\n",
        input.outcome.len(),
        input.regressors.len() + 2,
        fit.finite_rank(),
        report.used_solver,
        report.iterations,
        usize::from(report.fallback_used),
    );
    std::fs::write(destination.join("result.tsv"), metadata)?;
    Ok(())
}

fn next(
    input: &Input,
    workers: usize,
    tolerance: f64,
    solver: Solver,
    destination: &Path,
) -> Result<()> {
    let prepared = engine_inputs::prepare(&input.effects, &input.outcome);
    let team = ThreadTeam::new(workers)?;
    let options = Options {
        solver,
        tolerance,
        ..Options::for_estimator(Estimator::Ols)
    };
    let start = Instant::now();
    let design = Design::new(&prepared, Weights::Unit)?;
    let mut engine = Engine::new(&design, Estimator::Ols, &options, &team)?;
    let mut rhs = DenseMatrix::zeros(input.outcome.len(), input.regressors.len() + 2)?;
    for row in 0..rhs.nrows() {
        rhs[(row, 0)] = input.outcome[row];
        for (column, values) in input.regressors.iter().enumerate() {
            rhs[(row, column + 1)] = values[row];
        }
        rhs[(row, input.regressors.len() + 1)] = 1.;
    }
    let projection = engine.project(&rhs, &team)?;
    let seconds = start.elapsed().as_secs_f64();
    let k = input.regressors.len();
    let mut within = DenseMatrix::zeros(rhs.nrows(), k)?;
    let mut slots = Vec::new();
    for (column, values) in input.regressors.iter().enumerate() {
        slots.push(Slot {
            role: Role::Ordinary,
            reference_norm: values.iter().fold(0_f64, |norm, &x| norm.hypot(x)),
        });
        for row in 0..rhs.nrows() {
            within[(row, column)] = projection.residuals[(row, column + 1)];
        }
    }
    let outcome = projection.residuals.column(0)?;
    let fit = wls::solve(
        &Problem {
            design: &within,
            outcome: &outcome,
            weights: Weights::Unit,
            slots: &slots,
            has_absorbed_intercept: true,
        },
        Policy::default(),
        &team,
    )?;
    if fit.kept.len() != k {
        return Err("new benchmark ordinary coefficient was omitted".into());
    }
    io::write_values(&destination.join("coefficients.f64"), &fit.coefficients)?;
    io::write_values(&destination.join("residuals.f64"), &fit.residuals)?;
    let used = projection
        .diagnostics
        .iter()
        .map(|d| d.used.label())
        .collect::<Vec<_>>()
        .join(",");
    let iterations: usize = projection.diagnostics.iter().map(|d| d.iterations).sum();
    let setup: usize = projection
        .setup_diagnostics
        .iter()
        .flat_map(|d| &d.right_hand_sides)
        .map(|a| a.iterations)
        .sum();
    let fallbacks = projection
        .diagnostics
        .iter()
        .filter(|d| d.fallback.is_some())
        .count();
    let score = projection
        .certificate
        .relative_score
        .iter()
        .copied()
        .fold(0., f64::max);
    let metadata = format!(
        "rows\t{}\nrhs_count\t{}\nrank\t{k}\nprojection_seconds\t{seconds:.12}\nused\t{used}\niterations\t{iterations}\nsetup_iterations\t{setup}\nfallbacks\t{fallbacks}\nscore\t{score:.17e}\n",
        rhs.nrows(), rhs.ncols(),
    );
    std::fs::write(destination.join("result.tsv"), metadata)?;
    Ok(())
}
