//! Projection-only microbenchmark of fedemean on a synthetic worker-firm
//! panel built from raw (sparse) identifiers.
//!
//! ```text
//! cargo run --release -p fedemean --example demean -- \
//!     [--n 1000000] [--effects 2..5] [--cols 5] [--reps 5] [--threads 4] \
//!     [--tolerance 1e-8] [--weights]
//! ```
//!
//! Effects, in order: worker, firm (with worker mobility), year, occupation,
//! region. The first projection (`Projector::new` plus the first call, which
//! builds the structure) is timed separately from the repeated projections,
//! which start from a fresh copy of the columns; with `--weights` every
//! repetition writes slightly different weights into the same buffer, so each
//! one refreshes the weight state.

use fedemean::{
    ColumnsMut, Control, Design, Effect, Outcome, Projector, Request, Settings, Weights,
};
use std::time::{Duration, Instant};

type Error = Box<dyn std::error::Error + Send + Sync>;

const NAMES: [&str; 5] = ["worker", "firm", "year", "occupation", "region"];

struct Args {
    n: usize,
    effects: usize,
    cols: usize,
    reps: usize,
    threads: usize,
    tolerance: f64,
    weights: bool,
}

fn usage() -> &'static str {
    "usage: demean [--n N] [--effects 2..5] [--cols C] [--reps R] [--threads T] \
     [--tolerance TOL] [--weights]"
}

fn parse_args() -> Result<Args, String> {
    fn value<T: std::str::FromStr>(
        args: &mut impl Iterator<Item = String>,
        flag: &str,
    ) -> Result<T, String> {
        let text = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        text.replace('_', "")
            .parse()
            .map_err(|_| format!("{flag}: cannot parse {text:?}"))
    }
    let mut args = Args {
        n: 1_000_000,
        effects: 2,
        cols: 5,
        reps: 5,
        threads: 4,
        tolerance: 1e-8,
        weights: false,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--n" => args.n = value(&mut iter, "--n")?,
            "--effects" => args.effects = value(&mut iter, "--effects")?,
            "--cols" => args.cols = value(&mut iter, "--cols")?,
            "--reps" => args.reps = value(&mut iter, "--reps")?,
            "--threads" => args.threads = value(&mut iter, "--threads")?,
            "--tolerance" => args.tolerance = value(&mut iter, "--tolerance")?,
            "--weights" => args.weights = true,
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other:?}\n{}", usage())),
        }
    }
    if !(2..=5).contains(&args.effects) {
        return Err("--effects must be between 2 and 5".into());
    }
    if args.n < 100 || args.cols == 0 || args.reps == 0 || args.threads == 0 {
        return Err("--n must be at least 100; --cols, --reps and --threads positive".into());
    }
    Ok(args)
}

/// SplitMix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }

    fn chance(&mut self, p: f64) -> bool {
        self.uniform() < p
    }

    fn normal(&mut self) -> f64 {
        let u = self.uniform().max(1e-300);
        let v = self.uniform();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// A sparse raw identifier for index `i` of a family (distinct per family:
/// odd multiplication is a bijection of `u64`).
fn raw_id(i: usize, salt: u64) -> u64 {
    (i as u64 ^ salt).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

/// Raw identifiers of one panel: workers with several consecutive years,
/// moving between firms (larger firms more likely), changing occupation and
/// region now and then.
fn raw_panel(n: usize, seed: u64) -> [Vec<u64>; 5] {
    let mut rng = Rng(seed);
    let firms = (n / 40).max(10);
    let (years, occupations, regions) = (30, 300, 50);
    let mut raw: [Vec<u64>; 5] = std::array::from_fn(|_| Vec::with_capacity(n));
    let mut worker = 0;
    let skewed_firm = |rng: &mut Rng| {
        let u = rng.uniform();
        ((firms as f64) * u * u) as usize
    };
    while raw[0].len() < n {
        let spell = 2 + rng.below(11);
        let entry = rng.below(years - spell + 1);
        let mut firm = skewed_firm(&mut rng);
        let mut occupation = rng.below(occupations);
        let mut region = rng.below(regions);
        for t in 0..spell.min(n - raw[0].len()) {
            if t > 0 {
                if rng.chance(0.12) {
                    firm = skewed_firm(&mut rng);
                }
                if rng.chance(0.05) {
                    occupation = rng.below(occupations);
                }
                if rng.chance(0.03) {
                    region = rng.below(regions);
                }
            }
            raw[0].push(raw_id(worker, 0x5eed_0001));
            raw[1].push(raw_id(firm, 0x5eed_0002));
            raw[2].push(1990 + (entry + t) as u64);
            raw[3].push(10_000 + 7 * occupation as u64);
            raw[4].push(raw_id(region, 0x5eed_0005));
        }
        worker += 1;
    }
    raw
}

/// Dense codes `0..levels` of raw identifiers (sorted order); every level
/// is observed.
fn dense_codes(raw: &[u64]) -> (Vec<u32>, usize) {
    let mut levels = raw.to_vec();
    levels.sort_unstable();
    levels.dedup();
    let codes = raw
        .iter()
        .map(|id| levels.binary_search(id).expect("observed level") as u32)
        .collect();
    (codes, levels.len())
}

/// Observation-major columns: per column, a different mix of the effects
/// plus noise.
fn columns(codes: &[(Vec<u32>, usize)], n: usize, cols: usize, seed: u64) -> Vec<f64> {
    let mut rng = Rng(seed);
    let mut data = vec![0.0; n * cols];
    for j in 0..cols {
        let effects: Vec<(f64, Vec<f64>)> = codes
            .iter()
            .map(|(_, levels)| {
                let scale = 0.5 + 2.0 * rng.uniform();
                (scale, (0..*levels).map(|_| rng.normal()).collect())
            })
            .collect();
        for (i, row) in data.chunks_exact_mut(cols).enumerate() {
            let fit: f64 = codes
                .iter()
                .zip(&effects)
                .map(|((c, _), (scale, alpha))| scale * alpha[c[i] as usize])
                .sum();
            row[j] = fit + rng.normal();
        }
    }
    data
}

fn median(times: &mut [Duration]) -> Duration {
    times.sort_unstable();
    times[times.len() / 2]
}

/// Print one projection's route, accuracy and phase facts; `false` when it
/// declined.
fn describe(label: &str, outcome: &Outcome, projector: &Projector<'_>, elapsed: Duration) -> bool {
    let report = match outcome {
        Outcome::Projected(report) => report,
        Outcome::Declined { reason, facts, .. } => {
            println!("{label}: declined ({reason:?}) after {elapsed:.3?}; facts {facts:?}");
            return false;
        }
        _ => {
            println!("{label}: unknown outcome");
            return false;
        }
    };
    let moment = projector
        .columns()
        .iter()
        .fold(0.0_f64, |m, c| m.max(c.relative_moment));
    let facts = &report.facts;
    println!(
        "{label}: {:.4} s  route {:?}/{:?}/{:?}  iterations {}  max relative moment {:.2e}  certificate {}",
        elapsed.as_secs_f64(),
        report.route.reduction,
        report.route.method,
        report.route.preconditioner,
        facts.iterations,
        moment,
        if report.certificate.passed { "passed" } else { "failed" },
    );
    println!(
        "    structure {:.4} s  weights {:.4} s  rhs {:.4} s  pcg {:.4} s  residual {:.4} s  \
         (cells {}, reduced {}, lanes {}, corrections {})",
        facts.structure_seconds,
        facts.weights_seconds,
        facts.rhs_seconds,
        facts.pcg_seconds,
        facts.residual_seconds,
        facts.cells,
        facts.reduced,
        facts.lanes,
        facts.corrections,
    );
    true
}

fn run(args: &Args) -> Result<(), Error> {
    let started = Instant::now();
    let raw = raw_panel(args.n, 0x00de_3ea0);
    let codes: Vec<(Vec<u32>, usize)> = raw[..args.effects]
        .iter()
        .map(|ids| dense_codes(ids))
        .collect();
    drop(raw);
    let original = columns(&codes, args.n, args.cols, 0x0c01_5eed);
    let mut weights: Vec<f64> = {
        let mut rng = Rng(0x3e16_47a5);
        (0..args.n).map(|_| 0.5 + rng.uniform()).collect()
    };
    let levels: Vec<String> = codes
        .iter()
        .zip(NAMES)
        .map(|((_, levels), name)| format!("{name} {levels}"))
        .collect();
    println!(
        "panel: n {}  effects {} ({})  cols {}  weights {}  threads {}  tolerance {:e}  \
         generated in {:.2} s",
        args.n,
        args.effects,
        levels.join(", "),
        args.cols,
        if args.weights { "varying" } else { "unit" },
        args.threads,
        args.tolerance,
        started.elapsed().as_secs_f64()
    );

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build()?;
    pool.install(|| -> Result<(), Error> {
        let effects = codes
            .iter()
            .map(|(codes, levels)| Effect::new(codes, *levels))
            .collect();
        let design = Design::new(args.n, effects)?;
        let request = Request::new(args.tolerance, 1000);
        let control = Control::default();
        let mut data = original.clone();

        let started = Instant::now();
        let mut projector = Projector::new(design, Settings::default());
        let outcome = projector.project(
            if args.weights {
                Weights::Values(&weights)
            } else {
                Weights::Unit
            },
            ColumnsMut::new(&mut data, args.n, args.cols)?,
            None,
            &request,
            &control,
        )?;
        let first = started.elapsed();
        if !describe("first (new + project)", &outcome, &projector, first) {
            return Err("the first projection declined".into());
        }

        let mut times = Vec::with_capacity(args.reps);
        let mut last = None;
        for rep in 0..args.reps {
            data.copy_from_slice(&original);
            if args.weights {
                // A slight change of every weight, in the same buffer.
                let factor = 1.0 + 1e-3 * (rep + 1) as f64;
                for (i, w) in weights.iter_mut().enumerate() {
                    *w *= if i % 2 == 0 { factor } else { 1.0 / factor };
                }
            }
            let started = Instant::now();
            let outcome = projector.project(
                if args.weights {
                    Weights::Values(&weights)
                } else {
                    Weights::Unit
                },
                ColumnsMut::new(&mut data, args.n, args.cols)?,
                None,
                &request,
                &control,
            )?;
            let elapsed = started.elapsed();
            if !matches!(outcome, Outcome::Projected(_)) {
                describe(&format!("repetition {rep}"), &outcome, &projector, elapsed);
                return Err("a repeated projection declined".into());
            }
            times.push(elapsed);
            last = Some((outcome, elapsed));
        }
        let (min, max) = (
            times.iter().min().copied().unwrap_or_default(),
            times.iter().max().copied().unwrap_or_default(),
        );
        let median = median(&mut times);
        if let Some((outcome, elapsed)) = &last {
            describe("last repetition", outcome, &projector, *elapsed);
        }
        println!(
            "repeated: median {:.4} s over {} reps (min {:.4} s, max {:.4} s); first/median {:.2}",
            median.as_secs_f64(),
            args.reps,
            min.as_secs_f64(),
            max.as_secs_f64(),
            first.as_secs_f64() / median.as_secs_f64().max(f64::MIN_POSITIVE),
        );
        Ok(())
    })
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if let Err(error) = run(&args) {
        eprintln!("demean: {error}");
        std::process::exit(1);
    }
}
