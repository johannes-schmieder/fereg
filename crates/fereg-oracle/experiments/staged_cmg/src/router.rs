//! Deterministic structural starter, short useful probe, and one full-CMG commitment.
use crate::guarded::{self, Control, Error};
use crate::local::Forest;
use crate::preconditioner::Preconditioner;
use crate::problem::{norm, Problem};
use crate::recurrence::{PcgState, Tariff, LIMIT, TOL};
use crate::solve::ResultRow;
use std::time::Instant;

#[derive(Clone, Copy)]
pub struct Config {
    pub memory: usize,
    pub iterations: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            memory: guarded::MEMORY,
            iterations: LIMIT,
        }
    }
}
#[derive(Debug, PartialEq)]
pub struct Structure {
    pub active: usize,
    pub low: usize,
    pub edges: usize,
}
impl Structure {
    pub fn forest(&self, incidence: usize) -> bool {
        self.active > 0
            && self.low >= self.active.div_ceil(2)
            && self.edges <= incidence.saturating_mul(4)
    }
}
pub fn structure<F: FnMut(&'static str) -> bool>(
    p: &Problem,
    c: &mut Control<F>,
) -> Result<Structure, Error> {
    let mut degrees = guarded::filled(p.n, 0u8, c)?;
    let mut edges = 0;
    for e in p.offsets.windows(2) {
        c.check("routing worker")?;
        let d = e[1] - e[0];
        edges = guarded::add(edges, guarded::bytes(d, d.saturating_sub(1))? / 2)?;
        for (k, &v) in p.firms[e[0]..e[1]].iter().enumerate() {
            if k % 64 == 0 {
                c.check("routing incidences")?;
            }
            degrees[v] = degrees[v]
                .saturating_add(d.saturating_sub(1).min(3) as u8)
                .min(3);
        }
    }
    let mut active = 0;
    let mut low = 0;
    for (i, d) in degrees.into_iter().enumerate() {
        if i % 64 == 0 {
            c.check("routing degrees")?;
        }
        active += usize::from(d > 0);
        low += usize::from(d > 0 && d <= 2);
    }
    Ok(Structure { active, low, edges })
}
pub fn grace(previous: f64, current: f64, target: f64) -> bool {
    if !previous.is_finite()
        || !current.is_finite()
        || !target.is_finite()
        || target <= 0.
        || current <= target
        || current >= previous
    {
        return false;
    }
    let rate = (current / previous).ln() / 8.;
    let remaining = (target / current).ln() / rate;
    remaining.is_finite() && (0. ..=8.).contains(&remaining)
}
pub fn run(p: &Problem, rhs: &[Vec<f64>]) -> Result<ResultRow, String> {
    run_config(
        p,
        rhs,
        Config::default(),
        &mut Control { cancel: |_| false },
    )
}
pub fn run_config<F: FnMut(&'static str) -> bool>(
    p: &Problem,
    rhs: &[Vec<f64>],
    config: Config,
    c: &mut Control<F>,
) -> Result<ResultRow, String> {
    run_inner(p, rhs, config, c).map_err(|e| e.to_string())
}
fn run_inner<F: FnMut(&'static str) -> bool>(
    p: &Problem,
    rhs: &[Vec<f64>],
    config: Config,
    c: &mut Control<F>,
) -> Result<ResultRow, Error> {
    if config.iterations == 0 || config.iterations > LIMIT {
        return Err(Error::Fatal("invalid_iteration_cap".into()));
    }
    let total = Instant::now();
    let setup = Instant::now();
    let mut row = crate::protected::empty();
    let base = guarded::base(p, rhs)?;
    guarded::admit(base, config.memory)?;
    c.check("routing admission")?;
    let stats = structure(p, c)?;
    let graph_bound = guarded::graph_bound(p, stats.edges)?;
    let forest_scratch = guarded::add(
        guarded::bytes(p.n, 192)?,
        guarded::add(guarded::bytes(stats.edges, 2)?, 4096)?,
    )?;
    let forest_peak = guarded::add(guarded::add(base, graph_bound)?, forest_scratch)?;
    let choose_forest = stats.forest(p.firms.len()) && forest_peak <= config.memory;
    let mut peak = base;
    let mut graph = None;
    let mut pre = if choose_forest {
        peak = forest_peak;
        let t = Instant::now();
        let g = guarded::graph(p, stats.edges, c)?;
        row.graph_seconds += t.elapsed().as_secs_f64();
        row.graph_builds += 1;
        let forest = Forest::checked(&g, c)?;
        graph = Some(g);
        Preconditioner::Forest(forest)
    } else {
        let mut diagonal = guarded::filled(p.n, 0., c)?;
        for (dst, src) in diagonal.chunks_mut(64).zip(p.diagonal.chunks(64)) {
            c.check("diagonal copy")?;
            dst.copy_from_slice(src);
        }
        Preconditioner::Diagonal(diagonal)
    };
    let starter = if choose_forest { "forest" } else { "diagonal" };
    // Base already covers the retained four forest arrays; include its shared graph.
    let retained = if choose_forest {
        guarded::add(
            base,
            guarded::add(
                guarded::bytes(stats.edges, 16)?,
                guarded::add(guarded::bytes(p.n, 8)?, 256)?,
            )?,
        )?
    } else {
        base
    };
    row.decisions.push(format!("{{\"event\":\"route\",\"starter\":\"{starter}\",\"active\":{},\"low_degree\":{},\"edge_bound\":{},\"memory_bound\":{peak}}}",stats.active,stats.low,stats.edges));
    row.setup = setup.elapsed().as_secs_f64();
    let mut full = false;
    let mut disabled = false;
    for (index, b) in rhs.iter().enumerate() {
        c.check("rhs start")?;
        let clock = Instant::now();
        let started_full = full;
        let tariff = Tariff::new(p, 8 * p.n as u64);
        let mut state = PcgState::new(p, b, &mut pre, tariff, None).map_err(Error::Fatal)?;
        let mut previous = norm(b);
        let mut extended = false;
        let mut switched = 0;
        while !state.done && state.iterations < config.iterations {
            c.check("pcg iteration")?;
            let last = state.iterations + 1 == config.iterations;
            if let Err(e) = state.advance(p, b, &mut pre, last) {
                return Err(Error::Fatal(format!(
                    "rhs_{index}_{}",
                    state.failure_context(p, b, e)
                )));
            }
            if !state.done
                && !full
                && !disabled
                && matches!(state.iterations, 16 | 24 | 32)
                && !last
            {
                let actual = state.probe(p, b).map_err(Error::Fatal)?;
                if state.done {
                    break;
                }
                let extension = state.iterations == 24 && grace(previous, actual, TOL * norm(b));
                let commit = state.iterations == 32 || (state.iterations == 24 && !extension);
                row.decisions.push(format!("{{\"event\":\"probe\",\"rhs\":{index},\"iteration\":{},\"true_residual\":{actual:.17e},\"grace\":{extension},\"commit\":{commit}}}",state.iterations));
                previous = actual;
                extended |= extension;
                if commit {
                    let t = Instant::now();
                    match guarded::build(p, graph.as_ref(), retained, config.memory, c) {
                        Ok(built) => {
                            peak = peak.max(built.peak);
                            row.graph_seconds += built.graph_seconds;
                            row.graph_builds += built.graph_builds;
                            row.tail_levels = built.full.levels();
                            c.check("router installation")?;
                            pre = Preconditioner::Full(Box::new(built.full));
                            full = true;
                            switched = state.iterations;
                            row.upgrades += 1;
                            state.restart(p, b, &mut pre).map_err(Error::Fatal)?;
                        }
                        Err(Error::Memory { required, limit }) => {
                            disabled = true;
                            row.decisions.push(format!("{{\"event\":\"memory_refusal\",\"rhs\":{index},\"iteration\":{},\"required\":{required},\"limit\":{limit}}}",state.iterations));
                        }
                        Err(e) => return Err(e),
                    }
                    row.setup += t.elapsed().as_secs_f64();
                }
            }
        }
        if !state.done {
            return Err(Error::Fatal(format!(
                "iteration_limit_rhs_{index}_iterations_{}",
                state.iterations
            )));
        }
        row.iterations += state.iterations;
        row.certificates += state.certificates;
        row.apply_seconds += state.apply_seconds;
        row.residual = row.residual.max(state.residual);
        for v in &state.x {
            row.digest = (row.digest ^ v.to_bits()).wrapping_mul(1099511628211);
        }
        row.decisions.push(format!("{{\"event\":\"rhs\",\"rhs\":{index},\"started_full\":{started_full},\"iterations\":{},\"install_iteration\":{switched},\"grace\":{extended},\"build_disabled\":{disabled},\"memory_bound\":{peak},\"rhs_seconds\":{:.9}}}",state.iterations,clock.elapsed().as_secs_f64()));
        row.solutions.push(state.x);
    }
    // Include destructor costs of solver-owned setup and graph in complete time.
    drop(pre);
    drop(graph);
    row.seconds = total.elapsed().as_secs_f64();
    Ok(row)
}
