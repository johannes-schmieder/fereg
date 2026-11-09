//! Projection-only microbenchmark of fedemean on a synthetic worker-firm
//! panel built from raw (sparse) identifiers.
//!
//! ```text
//! cargo run --release -p fedemean --example demean -- \
//!     [--n 1000000] [--effects 1..6] [--cols 5] [--reps 5] [--threads 4] \
//!     [--tolerance 1e-8] [--weights] [--reduction auto|eliminate|full] \
//!     [--method auto|pcg|lsmr] [--precond auto|jacobi|block|identity|schwarz] \
//!     [--subdomains star|all] [--local auto|exact|approximate] \
//!     [--fallback decline|escalate] [--variant base|lowmob|chain] \
//!     [--slope worker|firm|...] [--slope-only]
//! ```
//!
//! `--precond block` is block-Jacobi with the default local solves (exact
//! dense solves of small blocks, Jacobi on large ones); `--precond schwarz`
//! is additive Schwarz with `--subdomains` and `--local`. `--method lsmr`
//! and Schwarz need `--features schwarz`.
//!
//! Effects, in order: worker, firm (with worker mobility), year, occupation,
//! region, industry (one per firm). `--slope firm` gives that effect a
//! varying slope on the trend `t = year - 1990` (`firm##c.t`; with
//! `--slope-only`, `firm#c.t` without the intercept); the columns then carry
//! a level-specific trend of that effect. Variants: `base` (mobility 0.12, larger
//! firms more likely), `lowmob` (mobility 0.02: many small worker-firm
//! components) and `chain` (firms on a line, uniform entry, mobility 0.3,
//! each move one or two firms along the line). The first projection (`Projector::new`
//! plus the first call, which builds the structure) is timed separately from
//! the repeated projections, which start from a fresh copy of the columns;
//! with `--weights` every repetition writes slightly different weights into
//! the same buffer, so each one refreshes the weight state.

use fedemean::{
    BlockSpec, Choice, ColumnsMut, Control, Design, Effect, Fallback, Method, Outcome,
    Preconditioner, Projector, Reduction, Request, SchwarzLocal, SchwarzSpec, Settings, Subdomains,
    Weights,
};
use std::time::{Duration, Instant};

type Error = Box<dyn std::error::Error + Send + Sync>;

const NAMES: [&str; 6] = ["worker", "firm", "year", "occupation", "region", "industry"];

struct Args {
    n: usize,
    effects: usize,
    cols: usize,
    reps: usize,
    threads: usize,
    tolerance: f64,
    weights: bool,
    reduction: Reduction,
    method: Method,
    preconditioner: Preconditioner,
    schwarz: SchwarzSpec,
    fallback: Fallback,
    variant: Variant,
    /// The effect with a slope on the trend, and whether it keeps its
    /// intercept.
    slope: Option<usize>,
    slope_intercept: bool,
}

/// The mobility pattern of the synthetic panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Variant {
    Base,
    LowMobility,
    Chain,
}

fn usage() -> &'static str {
    "usage: demean [--n N] [--effects 1..6] [--cols C] [--reps R] [--threads T] \
     [--tolerance TOL] [--weights] [--reduction auto|eliminate|full] [--method auto|pcg|lsmr] \
     [--precond auto|jacobi|block|identity|schwarz] [--subdomains star|all] \
     [--local auto|exact|approximate] [--fallback decline|escalate] \
     [--variant base|lowmob|chain] [--slope worker|firm|...] [--slope-only]"
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
        reduction: Reduction::Auto,
        method: Method::Auto,
        preconditioner: Preconditioner::Auto,
        schwarz: SchwarzSpec::default(),
        fallback: Fallback::Decline,
        variant: Variant::Base,
        slope: None,
        slope_intercept: true,
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
            "--reduction" => {
                args.reduction = match iter.next().as_deref() {
                    Some("auto") => Reduction::Auto,
                    Some("eliminate") => Reduction::EliminateLargest,
                    Some("full") => Reduction::Full,
                    _ => return Err(format!("--reduction: auto, eliminate or full\n{}", usage())),
                }
            }
            "--method" => {
                args.method = match iter.next().as_deref() {
                    Some("auto") => Method::Auto,
                    Some("pcg") => Method::Pcg,
                    Some("lsmr") => Method::Lsmr,
                    _ => return Err(format!("--method: auto, pcg or lsmr\n{}", usage())),
                }
            }
            "--precond" => {
                args.preconditioner = match iter.next().as_deref() {
                    Some("auto") => Preconditioner::Auto,
                    Some("jacobi") => Preconditioner::Jacobi,
                    Some("block") => Preconditioner::BlockJacobi(BlockSpec::default()),
                    Some("identity") => Preconditioner::Identity,
                    Some("schwarz") => Preconditioner::Schwarz(SchwarzSpec::default()),
                    _ => {
                        let options = "auto, jacobi, block, identity or schwarz";
                        return Err(format!("--precond: {options}\n{}", usage()));
                    }
                }
            }
            "--subdomains" => {
                args.schwarz.subdomains = match iter.next().as_deref() {
                    Some("star") => Subdomains::Star,
                    Some("all") => Subdomains::AllPairs,
                    _ => return Err(format!("--subdomains: star or all\n{}", usage())),
                }
            }
            "--local" => {
                args.schwarz.local = match iter.next().as_deref() {
                    Some("auto") => SchwarzLocal::Auto,
                    Some("exact") => SchwarzLocal::Exact,
                    Some("approximate") => SchwarzLocal::Approximate,
                    _ => {
                        let options = "auto, exact or approximate";
                        return Err(format!("--local: {options}\n{}", usage()));
                    }
                }
            }
            "--variant" => {
                args.variant = match iter.next().as_deref() {
                    Some("base") => Variant::Base,
                    Some("lowmob") => Variant::LowMobility,
                    Some("chain") => Variant::Chain,
                    _ => return Err(format!("--variant: base, lowmob or chain\n{}", usage())),
                }
            }
            "--fallback" => {
                args.fallback = match iter.next().as_deref() {
                    Some("decline") => Fallback::Decline,
                    Some("escalate") => Fallback::Escalate,
                    _ => return Err(format!("--fallback: decline or escalate\n{}", usage())),
                }
            }
            "--slope" => {
                let name = iter.next().unwrap_or_default();
                let effect = NAMES.iter().position(|&n| n == name);
                args.slope = Some(effect.ok_or_else(|| format!("--slope: one of {NAMES:?}"))?);
            }
            "--slope-only" => args.slope_intercept = false,
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other:?}\n{}", usage())),
        }
    }
    if let Preconditioner::Schwarz(spec) = &mut args.preconditioner {
        *spec = args.schwarz;
    }
    if !(1..=NAMES.len()).contains(&args.effects) {
        return Err(format!("--effects must be between 1 and {}", NAMES.len()));
    }
    if args.slope.is_some_and(|e| e >= args.effects) {
        return Err("--slope names an effect beyond --effects".into());
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
/// moving between firms (by `variant`), changing occupation and region now
/// and then; each firm belongs to one of 60 industries.
fn raw_panel(n: usize, seed: u64, variant: Variant) -> [Vec<u64>; 6] {
    let mut rng = Rng(seed);
    let firms = (n / 40).max(10);
    let (years, occupations, regions) = (30, 300, 50);
    let mut raw: [Vec<u64>; 6] = std::array::from_fn(|_| Vec::with_capacity(n));
    let mut worker = 0;
    let mobility = match variant {
        Variant::Base => 0.12,
        Variant::LowMobility => 0.02,
        Variant::Chain => 0.3,
    };
    let entry_firm = |rng: &mut Rng| match variant {
        Variant::Chain => rng.below(firms),
        _ => {
            let u = rng.uniform();
            ((firms as f64) * u * u) as usize
        }
    };
    let next_firm = |rng: &mut Rng, firm: usize| match variant {
        Variant::Chain => {
            let step = 1 + rng.below(2);
            if rng.chance(0.5) {
                (firm + step).min(firms - 1)
            } else {
                firm.saturating_sub(step)
            }
        }
        _ => entry_firm(rng),
    };
    while raw[0].len() < n {
        let spell = 2 + rng.below(11);
        let entry = rng.below(years - spell + 1);
        let mut firm = entry_firm(&mut rng);
        let mut occupation = rng.below(occupations);
        let mut region = rng.below(regions);
        for t in 0..spell.min(n - raw[0].len()) {
            if t > 0 {
                if rng.chance(mobility) {
                    firm = next_firm(&mut rng, firm);
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
            raw[5].push(raw_id(firm % 60, 0x5eed_0006));
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
/// (with `slope`, that effect's level-specific trend too) plus noise.
fn columns(
    codes: &[(Vec<u32>, usize)],
    slope: Option<(usize, &[f64])>,
    (n, cols, seed): (usize, usize, u64),
) -> Vec<f64> {
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
        let trends: Vec<f64> = slope.map_or_else(Vec::new, |(e, _)| {
            (0..codes[e].1).map(|_| 0.1 * rng.normal()).collect()
        });
        for (i, row) in data.chunks_exact_mut(cols).enumerate() {
            let mut fit: f64 = codes
                .iter()
                .zip(&effects)
                .map(|((c, _), (scale, alpha))| scale * alpha[c[i] as usize])
                .sum();
            if let Some((e, t)) = slope {
                fit += trends[codes[e].0[i] as usize] * t[i];
            }
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
        "{label}: {:.4} s  route {:?}/{:?}/{:?}  escalations {}  iterations {}  \
         max relative moment {:.2e}  certificate {}",
        elapsed.as_secs_f64(),
        report.route.reduction,
        report.route.method,
        report.route.preconditioner,
        report.route.escalations,
        facts.iterations,
        moment,
        if report.certificate.passed {
            "passed"
        } else {
            "failed"
        },
    );
    println!(
        "    structure {:.4} s  weights {:.4} s  blocks {:.4} s  rhs {:.4} s  pcg {:.4} s  \
         residual {:.4} s  (cells {}, reduced {}, lanes {}, corrections {}, workspace {} MB)",
        facts.structure_seconds,
        facts.weights_seconds,
        facts.block_seconds,
        facts.rhs_seconds,
        facts.pcg_seconds,
        facts.residual_seconds,
        facts.cells,
        facts.reduced,
        facts.lanes,
        facts.corrections,
        report.workspace_bytes / (1 << 20),
    );
    true
}

fn run(args: &Args) -> Result<(), Error> {
    let started = Instant::now();
    let raw = raw_panel(args.n, 0x00de_3ea0, args.variant);
    let codes: Vec<(Vec<u32>, usize)> = raw[..args.effects]
        .iter()
        .map(|ids| dense_codes(ids))
        .collect();
    let trend: Vec<f64> = raw[2].iter().map(|&year| (year - 1990) as f64).collect();
    drop(raw);
    let slope = args.slope.map(|e| (e, trend.as_slice()));
    let original = columns(&codes, slope, (args.n, args.cols, 0x0c01_5eed));
    let mut weights: Vec<f64> = {
        let mut rng = Rng(0x3e16_47a5);
        (0..args.n).map(|_| 0.5 + rng.uniform()).collect()
    };
    let levels: Vec<String> = codes
        .iter()
        .zip(NAMES)
        .enumerate()
        .map(|(e, ((_, levels), name))| match args.slope {
            Some(s) if s == e && args.slope_intercept => format!("{name}##c.t {levels}"),
            Some(s) if s == e => format!("{name}#c.t {levels}"),
            _ => format!("{name} {levels}"),
        })
        .collect();
    println!(
        "panel: {:?} n {}  effects {} ({})  cols {}  weights {}  threads {}  tolerance {:e}  \
         reduction {:?}  method {:?}  precond {:?}  fallback {:?}  generated in {:.2} s",
        args.variant,
        args.n,
        args.effects,
        levels.join(", "),
        args.cols,
        if args.weights { "varying" } else { "unit" },
        args.threads,
        args.tolerance,
        args.reduction,
        args.method,
        args.preconditioner,
        args.fallback,
        started.elapsed().as_secs_f64()
    );

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build()?;
    pool.install(|| -> Result<(), Error> {
        let effects = codes
            .iter()
            .enumerate()
            .map(|(e, (codes, levels))| {
                let effect = Effect::new(codes, *levels);
                match args.slope {
                    Some(s) if s == e => effect.with_slopes(args.slope_intercept, [&trend[..]]),
                    _ => Ok(effect),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let design = Design::new(args.n, effects)?;
        let request = Request::new(args.tolerance, 1000);
        let control = Control::default();
        let mut data = original.clone();

        let started = Instant::now();
        let settings = Settings {
            choice: Choice {
                reduction: args.reduction,
                method: args.method,
                preconditioner: args.preconditioner,
                fallback: args.fallback,
            },
            ..Settings::default()
        };
        let mut projector = Projector::new(design, settings);
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
