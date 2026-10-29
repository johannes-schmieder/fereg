//! Independent public-API slope-rank witness; never imported by production.
//! The companion Python runner generates exact-input rational oracles.
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Fixture {
    id: String,
    levels: Vec<u32>,
    slopes: [Vec<f64>; 2],
    weights: Vec<f64>,
    y: Vec<f64>,
}

fn input_digest(f: &Fixture) -> String {
    let mut hash = Sha256::new();
    hash.update((f.levels.len() as u64).to_le_bytes());
    for &value in &f.levels {
        hash.update(u64::from(value).to_le_bytes());
    }
    for column in [&f.slopes[0], &f.slopes[1], &f.weights, &f.y] {
        hash.update((column.len() as u64).to_le_bytes());
        for &value in column {
            hash.update(value.to_bits().to_le_bytes());
        }
    }
    format!("{:x}", hash.finalize())
}

fn fork(f: &Fixture) -> Value {
    use within_fork::{Effect, LsmrOptions, PreconditionerConfig, SlopeScreening, Solver};
    let run = || -> Result<Value, String> {
        let effect = Effect::new(&f.levels, true, f.slopes.iter().map(Vec::as_slice))
            .map_err(|e| e.to_string())?;
        let solver = Solver::new_with_screening(
            vec![effect],
            Some(f.weights.clone()),
            PreconditionerConfig::Off,
            SlopeScreening::Disabled,
        )
        .map_err(|e| e.to_string())?
        .with_reproducible()
        .map_err(|e| e.to_string())?;
        let result = solver
            .solve(
                &f.y,
                &LsmrOptions {
                    tol: 1e-12,
                    maxiter: 100,
                    local_size: Some(8),
                },
            )
            .map_err(|e| e.to_string())?;
        Ok(json!({
            "x": result.x,
            "demeaned": result.demeaned,
            "unidentified": result.unidentified.iter().map(|a| [a.channel.term, a.channel.column, a.level as usize]).collect::<Vec<_>>(),
            "layout": (0..result.x.len()).map(|i| { let a = result.layout.address(i).unwrap(); [a.channel.term, a.channel.column, a.level as usize] }).collect::<Vec<_>>(),
            "converged": result.converged,
            "iterations": result.iterations,
            "normal_ratio": result.residual,
        }))
    };
    run().unwrap_or_else(|error| json!({"error": error}))
}

fn candidate(f: &Fixture) -> Value {
    use within_candidate::{Effect, LsmrOptions, PreconditionerConfig, Solver};
    let run = || -> Result<Value, String> {
        let effect = Effect::new(&f.levels, true, f.slopes.iter().map(Vec::as_slice))
            .map_err(|e| e.to_string())?;
        let solver = Solver::new(
            vec![effect],
            Some(f.weights.as_slice()),
            PreconditionerConfig::Off,
        )
        .map_err(|e| e.to_string())?;
        let result = solver
            .solve(
                &f.y,
                &LsmrOptions {
                    tol: 1e-12,
                    maxiter: 100,
                    local_size: Some(8),
                },
            )
            .map_err(|e| e.to_string())?;
        Ok(json!({
            "x": result.x,
            "demeaned": result.demeaned,
            "unidentified": result.unidentified.iter().map(|a| [a.channel.term, a.channel.column, a.level as usize]).collect::<Vec<_>>(),
            "layout": (0..result.x.len()).map(|i| { let a = result.layout.address(i).unwrap(); [a.channel.term, a.channel.column, a.level as usize] }).collect::<Vec<_>>(),
            "converged": result.converged,
            "iterations": result.iterations,
            "normal_ratio": result.residual,
        }))
    };
    run().unwrap_or_else(|error| json!({"error": error}))
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 4, "usage: boundary ARM WORKERS FIXTURES.json");
    let arm = &args[1];
    let workers: usize = args[2].parse().unwrap();
    let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(&args[3]).unwrap()).unwrap();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .unwrap();
    pool.install(|| {
        for fixture in &fixtures {
            for repeat in 0..2 {
                let result = match arm.as_str() {
                    "fork" => fork(fixture),
                    "candidate" => candidate(fixture),
                    _ => panic!("unknown arm"),
                };
                println!("{}", json!({"fixture": fixture.id, "arm": arm, "workers": workers, "repeat": repeat, "input_sha256": input_digest(fixture), "result": result}));
            }
        }
    });
}
