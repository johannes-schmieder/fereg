//! Prepared benchmark source only: no execution authorized by its presence.
//! Identical fixture/algebra across base, API, and cancellation feature variants.
//! All scientific streaming/hash work occurs outside external construction/solve timers.

use std::hint::black_box;
use std::io::{self, BufWriter, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[cfg(any(
    all(feature = "base", feature = "api"),
    all(feature = "base", feature = "cancel"),
    all(feature = "api", feature = "cancel"),
    not(any(feature = "base", feature = "api", feature = "cancel"))
))]
compile_error!("select exactly one of base, api, cancel");

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SMALL: usize = 8192;
const LARGE: usize = 4_194_304;
const TOLERANCE: f64 = 1e-12;
const WITNESS: f64 = 1e-8;
const RHS: [usize; 4] = [0, 1, 0, 1];
const ENTRYPOINT: &str = if cfg!(feature = "api") {
    "controlled_default"
} else if cfg!(feature = "cancel") {
    "cancellable_false_atomic"
} else {
    "legacy"
};
use within::{
    BuildWarning, CoefficientAddress, CoefficientLayout, Effect, LocalSolverConfig, LsmrOptions,
    PreconditionerConfig, ReductionStrategy, SolveResult, Solver,
};

struct Stream<W: Write>(W);

impl<W: Write> Stream<W> {
    fn header(&mut self, name: &str, bytes: usize) -> io::Result<()> {
        self.0.write_all(&(name.len() as u64).to_le_bytes())?;
        self.0.write_all(name.as_bytes())?;
        self.0.write_all(&(bytes as u64).to_le_bytes())
    }

    fn words(&mut self, name: &str, values: impl ExactSizeIterator<Item = u64>) -> io::Result<()> {
        self.header(name, values.len() * 8)?;
        for value in values {
            self.0.write_all(&value.to_le_bytes())?;
        }
        Ok(())
    }

    fn floats(&mut self, name: &str, values: &[f64]) -> io::Result<()> {
        self.words(name, values.iter().map(|v| v.to_bits()))
    }

    fn text(&mut self, name: &str, value: &str) -> io::Result<()> {
        self.header(name, value.len())?;
        self.0.write_all(value.as_bytes())
    }

    fn scalar(&mut self, name: &str, value: u64) -> io::Result<()> {
        self.words(name, [value].into_iter())
    }
}

struct Fixture {
    levels: [Vec<u32>; 3],
    slopes: Vec<Vec<f64>>,
    weights: Vec<f64>,
    responses: [Vec<f64>; 2],
}

fn planted_residual(row: usize, rhs: usize) -> f64 {
    let r = row % 8;
    let sign = match rhs {
        0 => [1.0, -1.0, 1.0, -1.0][r & 3],
        1 => [1.0, -1.0, -1.0, 1.0][r & 3],
        _ => unreachable!(),
    };
    sign * if r < 4 { 1.5 } else { -1.0 }
}

impl Fixture {
    fn new(n: usize, slope_count: usize) -> Self {
        let mut levels = std::array::from_fn(|_| Vec::with_capacity(n));
        let mut slopes = vec![Vec::with_capacity(n); slope_count];
        let mut weights = Vec::with_capacity(n);
        let mut responses = std::array::from_fn(|_| Vec::with_capacity(n));
        let b_levels = if n == 8192 { 16 } else { 64 };
        for row in 0..n {
            let a = row / (b_levels * 8);
            let b = (row / 8) % b_levels;
            let r = row % 8;
            let h = (a + 3 * b + (r >> 2)) % b_levels;
            levels[0].push((a * 13 + 5) as u32);
            levels[1].push((b * 7 + 2) as u32);
            levels[2].push((h * 3 + 1) as u32);
            let loading = 1.0 + 0.5 * (r >> 2) as f64;
            // Disjoint positive support makes the within-term Gram diagonal.
            // The unused fourth row pair also avoids a constant slope sum.
            for (column, slope) in slopes.iter_mut().enumerate() {
                slope.push(if r & 3 == column { loading } else { 0.0 });
            }
            weights.push(1.0 + ((a + 3 * b) % 4) as f64 / 4.0);
            for (rhs, response) in responses.iter_mut().enumerate() {
                let mut fitted = ((b + rhs * 2) % 9) as f64 / 8.0 - 0.5
                    + ((h + rhs * 3) % 11) as f64 / 8.0
                    - 0.625;
                if slope_count == 0 {
                    fitted += ((a + rhs * 5) % 17) as f64 / 16.0 - 0.5;
                } else if r & 3 < slope_count {
                    fitted += loading * (((a + 3 * (r & 3) + rhs * 5) % 17) as f64 / 16.0 - 0.5);
                }
                response.push(fitted + planted_residual(row, rhs));
            }
        }
        Self {
            levels,
            slopes,
            weights,
            responses,
        }
    }

    fn effects(&self) -> Vec<Effect<'_>> {
        vec![
            Effect::new(
                &self.levels[0],
                self.slopes.is_empty(),
                self.slopes.iter().map(Vec::as_slice),
            )
            .unwrap(),
            Effect::new(&self.levels[1], true, []).unwrap(),
            Effect::new(&self.levels[2], true, []).unwrap(),
        ]
    }

    fn stream(
        &self,
        output: &mut Stream<impl Write>,
        prefix: &str,
        weighted: bool,
    ) -> io::Result<()> {
        for (term, levels) in self.levels.iter().enumerate() {
            output.words(
                &format!("{prefix}/levels{term}"),
                levels.iter().map(|v| u64::from(*v)),
            )?;
        }
        for (column, slope) in self.slopes.iter().enumerate() {
            output.floats(&format!("{prefix}/slope{column}"), slope)?;
        }
        if weighted {
            output.floats(&format!("{prefix}/weights"), &self.weights)?;
        }
        for (column, response) in self.responses.iter().enumerate() {
            output.floats(&format!("{prefix}/response{column}"), response)?;
        }
        Ok(())
    }
}

fn address(address: CoefficientAddress) -> [u64; 3] {
    [
        address.channel.term as u64,
        address.channel.column as u64,
        u64::from(address.level),
    ]
}

fn shared(
    output: &mut Stream<impl Write>,
    prefix: &str,
    layout: &CoefficientLayout,
    unidentified: &[CoefficientAddress],
    warnings: &[BuildWarning],
) -> io::Result<()> {
    output.scalar(&format!("{prefix}/layout/n_dofs"), layout.n_dofs() as u64)?;
    output.scalar(&format!("{prefix}/layout/n_terms"), layout.n_terms() as u64)?;
    let terms: Vec<u64> = (0..layout.n_terms())
        .flat_map(|term| {
            [
                layout.n_levels(term).unwrap() as u64,
                layout.n_columns(term).unwrap() as u64,
            ]
        })
        .collect();
    output.words(&format!("{prefix}/layout/terms"), terms.into_iter())?;
    let slots: Vec<u64> = (0..layout.n_dofs())
        .flat_map(|slot| {
            let at = layout.address(slot).unwrap();
            assert_eq!(layout.index(at), Some(slot));
            address(at)
        })
        .collect();
    output.words(&format!("{prefix}/layout/addresses"), slots.into_iter())?;
    let dropped: Vec<u64> = unidentified.iter().copied().flat_map(address).collect();
    output.words(&format!("{prefix}/unidentified"), dropped.into_iter())?;
    output.scalar(&format!("{prefix}/warning_count"), warnings.len() as u64)?;
    for (index, warning) in warnings.iter().enumerate() {
        match warning {
            BuildWarning::UnscalableComponent {
                pair,
                iterations,
                violation,
            } => {
                output.words(
                    &format!("{prefix}/warning{index}/unscalable"),
                    [
                        pair.rows.term as u64,
                        pair.rows.column as u64,
                        pair.cols.term as u64,
                        pair.cols.column as u64,
                        *iterations as u64,
                        violation.to_bits(),
                    ]
                    .into_iter(),
                )?;
            }
            _ => panic!("unrecognized scientific warning: {warning:?}"),
        }
    }
    Ok(())
}

struct HashWriter(u64);
impl Default for HashWriter {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn new_solver<'a>(
    fixture: &'a Fixture,
    config: &PreconditionerConfig,
    _query: &(dyn Fn() -> bool + Send + Sync),
) -> Result<Solver<'a>> {
    #[cfg(feature = "cancel")]
    return Ok(Solver::new_cancellable(
        fixture.effects(),
        Some(&fixture.weights),
        config,
        within::Cancellation::new(_query),
    )?);
    #[cfg(not(feature = "cancel"))]
    Ok(Solver::new(
        fixture.effects(),
        Some(&fixture.weights),
        config,
    )?)
}

struct Attempt {
    solution: SolveResult,
    #[cfg(feature = "api")]
    stop_reason: within::LsmrStopReason,
}

impl Attempt {
    fn reason(&self) -> Result<Option<&'static str>> {
        #[cfg(feature = "api")]
        return match self.stop_reason {
            within::LsmrStopReason::ResidualTolerance => Ok(Some("ResidualTolerance")),
            within::LsmrStopReason::NormalEquationTolerance => Ok(Some("NormalEquationTolerance")),
            other => Err(format!("unexpected controlled stop: {other:?}").into()),
        };
        #[cfg(not(feature = "api"))]
        Ok(None)
    }
}

fn solve(
    solver: &Solver<'_>,
    y: &[f64],
    options: &LsmrOptions,
    _query: &(dyn Fn() -> bool + Send + Sync),
) -> Result<Attempt> {
    #[cfg(feature = "api")]
    {
        let result =
            solver.solve_controlled(black_box(y), options, within::SolveControl::default())?;
        Ok(Attempt {
            solution: result.solution,
            stop_reason: result.stop_reason,
        })
    }
    #[cfg(feature = "cancel")]
    {
        Ok(Attempt {
            solution: solver.solve_cancellable(
                black_box(y),
                options,
                within::Cancellation::new(_query),
            )?,
        })
    }
    #[cfg(feature = "base")]
    {
        Ok(Attempt {
            solution: solver.solve(black_box(y), options)?,
        })
    }
}

fn scientific(
    output: &mut Stream<impl Write>,
    prefix: &str,
    result: &SolveResult,
    rhs: usize,
) -> io::Result<f64> {
    shared(
        output,
        prefix,
        &result.layout,
        &result.unidentified,
        &result.warnings,
    )?;
    output.floats(&format!("{prefix}/coefficients"), &result.x)?;
    output.floats(&format!("{prefix}/demeaned"), &result.demeaned)?;
    output.scalar(&format!("{prefix}/converged"), u64::from(result.converged))?;
    output.scalar(&format!("{prefix}/iterations"), result.iterations as u64)?;
    output.floats(&format!("{prefix}/residual"), &[result.residual])?;
    let error = result
        .demeaned
        .iter()
        .enumerate()
        .map(|(row, value)| (value - planted_residual(row, rhs)).abs())
        .fold(0.0f64, f64::max);
    output.floats(&format!("{prefix}/planted_residual_error"), &[error])?;
    if !result.converged
        || !result.residual.is_finite()
        || error > WITNESS
        || !result
            .x
            .iter()
            .chain(&result.demeaned)
            .all(|v| v.is_finite())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("independent scientific witness failed: {error:e}"),
        ));
    }
    Ok(error)
}

struct Args {
    n: usize,
    slopes: usize,
    mode: String,
    measure: bool,
    stream: bool,
}
impl Args {
    fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let n = args.next().ok_or("usage: binary <8192|4194304> <0|3> <off|diagonal|additive> [--measure|--science-stream]")?.parse()?;
        let slopes = args.next().ok_or("missing slopes")?.parse()?;
        let mode = args.next().ok_or("missing preconditioner")?;
        let flag = args.next();
        if !matches!(n, SMALL | LARGE)
            || !matches!(slopes, 0 | 3)
            || !matches!(mode.as_str(), "off" | "diagonal" | "additive")
            || !matches!(
                flag.as_deref(),
                None | Some("--measure") | Some("--science-stream")
            )
            || args.next().is_some()
        {
            return Err("undeclared case or flag".into());
        }
        Ok(Self {
            n,
            slopes,
            mode,
            measure: flag.as_deref() == Some("--measure"),
            stream: flag.as_deref() == Some("--science-stream"),
        })
    }
}

fn run(args: Args) -> Result<()> {
    let fixture = Fixture::new(args.n, args.slopes);
    let config = match args.mode.as_str() {
        "off" => PreconditionerConfig::Off,
        "diagonal" => PreconditionerConfig::Diagonal,
        "additive" => {
            let mut local_solver = LocalSolverConfig::default();
            local_solver.approx_chol.seed = 7;
            PreconditionerConfig::Additive {
                local_solver,
                reduction: ReductionStrategy::ParallelReduction,
            }
        }
        _ => unreachable!(),
    };
    let options = LsmrOptions {
        tol: TOLERANCE,
        maxiter: 1000,
        local_size: Some(8),
    };
    // Opaque live storage prevents a constant-false query from replacing atomic loads.
    let cancelled = AtomicBool::new(black_box(false));
    let query = || black_box(&cancelled).load(Ordering::Relaxed);
    let started = args.measure.then(Instant::now);
    let solver = new_solver(&fixture, &config, &query)?;
    let build_ns = started.map(|t| t.elapsed().as_nanos());
    let mut inputs = Stream(HashWriter::default());
    fixture.stream(&mut inputs, "input", true)?;
    let input_hash = inputs.0 .0;
    let mut previous = None;
    let mut final_group = Vec::new();
    let mut elapsed_ns = 0_u128;
    let mut science_hash = 0_u64;
    let mut maximum_error = 0.0_f64;
    // Exactly one warmup group and one retained group, in either timer mode.
    for group in 0..2 {
        let mut solved = Vec::with_capacity(RHS.len());
        let mut group_ns = 0_u128;
        for rhs in RHS {
            let started = (args.measure && group == 1).then(Instant::now);
            let result = solve(&solver, &fixture.responses[rhs], &options, &query)?;
            group_ns += started.map_or(0, |t| t.elapsed().as_nanos());
            let reason = result.reason()?;
            solved.push((result.solution, reason));
        }
        // Output validation/hashing and result dropping never enter the solve timer.
        let mut signature = Stream(HashWriter::default());
        for (index, ((result, _), rhs)) in solved.iter().zip(RHS).enumerate() {
            let b = if args.n == SMALL { 16 } else { 64 };
            let a = args.n / (b * 8);
            let dofs = a * args.slopes.max(1) + 2 * b;
            if result.demeaned.len() != args.n
                || result.x.len() != dofs
                || result.layout.n_dofs() != dofs
                || result.layout.n_terms() != 3
                || result.layout.n_levels(0) != Some(a)
                || result.layout.n_columns(0) != Some(args.slopes.max(1))
                || [1, 2].iter().any(|&t| {
                    result.layout.n_levels(t) != Some(b) || result.layout.n_columns(t) != Some(1)
                })
            {
                return Err("scientific result dimensions differ from the declared design".into());
            }
            maximum_error = maximum_error.max(scientific(
                &mut signature,
                &format!("rhs{index}"),
                result,
                rhs,
            )?);
        }
        if previous.is_some_and(|value| value != signature.0 .0) {
            return Err("fixed-worker scientific signature changed between groups".into());
        }
        previous = Some(signature.0 .0);
        if group == 1 {
            final_group = solved;
            elapsed_ns = group_ns;
            science_hash = signature.0 .0;
        }
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err("false cancellation flag changed".into());
    }
    if args.stream {
        let mut output = Stream(BufWriter::with_capacity(1 << 20, io::stdout()));
        output.scalar("schema_version", 1)?;
        output.scalar("input_fnv64", input_hash)?;
        output.scalar("science_fnv64", science_hash)?;
        output.text("aux/entrypoint", ENTRYPOINT)?;
        let reasons: Vec<_> = final_group.iter().filter_map(|(_, r)| *r).collect();
        output.text("aux/stop_reasons", &reasons.join(","))?;
        output.text("preconditioner", &args.mode)?;
        output.words(
            "configuration",
            [
                args.n as u64,
                1,
                args.slopes as u64,
                1,
                7,
                options.tol.to_bits(),
                options.maxiter as u64,
                options.local_size.unwrap() as u64,
                4,
                1,
            ]
            .into_iter(),
        )?;
        fixture.stream(&mut output, "input", true)?;
        for (index, ((result, _), rhs)) in final_group.iter().zip(RHS).enumerate() {
            scientific(&mut output, &format!("rhs{index}"), result, rhs)?;
        }
        output.0.flush()?;
        return Ok(());
    }
    let iterations: Vec<_> = final_group.iter().map(|(r, _)| r.iterations).collect();
    let residuals: Vec<_> = final_group.iter().map(|(r, _)| r.residual).collect();
    let reasons: Vec<_> = final_group.iter().filter_map(|(_, r)| *r).collect();
    let ns = |x: Option<u128>| x.map_or_else(|| "null".into(), |x| x.to_string());
    println!(concat!("{{\"entrypoint\":\"{}\",\"rows\":{},\"slopes\":{},\"mode\":\"{}\",\"workers\":1,\"weighted\":true,",
        "\"tolerance\":1e-12,\"witness\":1e-8,\"rhs\":4,\"warmups\":1,",
        "\"input_fnv64\":\"{:016x}\",\"science_fnv64\":\"{:016x}\",\"maximum_error\":{:e},",
        "\"iterations\":{:?},\"recurrence_residuals\":{:?},\"controlled_stop_reasons\":{:?},",
        "\"build_ns\":{},\"elapsed_ns\":{},\"total_ns\":{}}}"), ENTRYPOINT, args.n, args.slopes, args.mode,
        input_hash, science_hash, maximum_error, iterations, residuals, reasons,
        ns(build_ns), ns(args.measure.then_some(elapsed_ns)), ns(build_ns.map(|n| n + elapsed_ns)));
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse()?;
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build()?;
    pool.install(|| run(args).map_err(|error| error.to_string()))
        .map_err(Into::into)
}
