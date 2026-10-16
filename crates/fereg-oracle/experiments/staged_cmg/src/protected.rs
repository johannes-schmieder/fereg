//! Deterministic prepaid optional service, with the cheap recurrence left intact.
use crate::builder::{BuildTask, Ready, CAP};
use crate::local::Forest;
use crate::preconditioner::Preconditioner;
use crate::problem::Problem;
use crate::recurrence::{PcgState, Tariff, LIMIT};
use crate::solve::ResultRow;
use std::fmt::Write;
use std::sync::Arc;
use std::time::Instant;

pub const ARMS: [&str; 14] = [
    "diagonal-q14",
    "diagonal-q1",
    "forest-q14",
    "forest-q1",
    "full-controlled",
    "diagonal-p14",
    "diagonal-p1",
    "forest-p14",
    "forest-p1",
    "diagonal-c14",
    "diagonal-c1",
    "forest-c14",
    "forest-c1",
    "full-chunked",
];
#[derive(Clone, Copy)]
pub struct Config {
    pub denominator: u64,
    pub memory: usize,
    pub iterations: usize,
}
impl Config {
    pub fn for_arm(arm: &str) -> Self {
        Self {
            denominator: if arm.ends_with("14") { 4 } else { 1 },
            memory: 1 << 30,
            iterations: LIMIT,
        }
    }
}
#[derive(Default)]
struct Ledger {
    inc: u64,
    optional: u64,
    build: u64,
    challenger: u64,
    control: u64,
    denominator: u64,
}
impl Ledger {
    fn allowance(&self) -> u64 {
        if self.denominator == 0 {
            0
        } else {
            self.inc / self.denominator
        }
    }
    fn available(&self) -> u64 {
        self.allowance()
            .checked_sub(self.optional)
            .expect("prepaid envelope")
    }
    fn spend(&mut self, n: u64, phase: usize) -> bool {
        if n > self.available() {
            return false;
        }
        self.optional = self.optional.checked_add(n).unwrap();
        let cell = match phase {
            0 => &mut self.build,
            1 => &mut self.challenger,
            _ => &mut self.control,
        };
        *cell = cell.checked_add(n).unwrap();
        true
    }
    fn refund_build(&mut self, n: u64) {
        self.build -= n;
        self.optional -= n;
    }
}
fn empty() -> ResultRow {
    ResultRow {
        seconds: 0.,
        setup: 0.,
        graph_seconds: 0.,
        apply_seconds: 0.,
        iterations: 0,
        upgrades: 0,
        certificates: 0,
        residual: 0.,
        digest: 14695981039346656037,
        graph_builds: 0,
        coarse_n: 0,
        solutions: vec![],
        decisions: vec![],
        tail_levels: 0,
    }
}
fn publish(row: &mut ResultRow, state: PcgState) {
    row.residual = row.residual.max(state.residual);
    for v in &state.x {
        row.digest = (row.digest ^ v.to_bits()).wrapping_mul(1099511628211);
    }
    row.solutions.push(state.x);
}
fn state_counts(row: &mut ResultRow, s: &PcgState) {
    row.iterations += s.iterations;
    row.certificates += s.certificates;
    row.apply_seconds += s.apply_seconds;
}
fn memory_base(p: &Problem, rhs: &[Vec<f64>], forest: bool, edges: usize) -> Result<usize, String> {
    // Input capacities, RHS bank, all outputs, both 6-vector recurrences, projection scratch,
    // cheap preconditioner and shared graph. Roots/channel bookkeeping: fixed 64 KiB in builder.
    let input = [
        p.offsets.capacity(),
        p.firms.capacity(),
        p.weights.capacity(),
        p.totals.capacity(),
        p.diagonal.capacity(),
        p.labels.capacity(),
        p.sizes.capacity(),
    ];
    let mut bytes = input
        .into_iter()
        .try_fold(0usize, |a, b| a.checked_add(b.checked_mul(8)?))
        .ok_or("memory_overflow")?;
    for b in rhs {
        bytes = bytes
            .checked_add(b.capacity().checked_mul(8).ok_or("memory_overflow")?)
            .ok_or("memory_overflow")?;
    }
    for b in [
        p.n.checked_mul((rhs.len() + 13) * 8)
            .ok_or("memory_overflow")?,
        p.n.checked_mul(if forest { 40 } else { 8 })
            .ok_or("memory_overflow")?,
        edges.checked_mul(16).ok_or("memory_overflow")?,
        rhs.len().checked_mul(8256).ok_or("memory_overflow")?,
        256,
    ] {
        bytes = bytes.checked_add(b).ok_or("memory_overflow")?;
    }
    Ok(bytes)
}
fn retire(
    task: &mut BuildTask<'_>,
    ledger: &mut Ledger,
    row: &mut ResultRow,
) -> Result<(), String> {
    let t = Instant::now();
    task.abandon()?;
    row.setup += t.elapsed().as_secs_f64();
    ledger.refund_build(task.report.snapshot.issued - task.report.snapshot.charged);
    row.graph_builds += task.report.graph_builds;
    row.graph_seconds += task.report.graph_seconds;
    Ok(())
}
pub fn run(p: &Problem, rhs: &[Vec<f64>], arm: &str) -> Result<ResultRow, String> {
    run_config(p, rhs, arm, Config::for_arm(arm))
}
pub fn run_config(
    p: &Problem,
    rhs: &[Vec<f64>],
    arm: &str,
    config: Config,
) -> Result<ResultRow, String> {
    assert!(ARMS.contains(&arm));
    if config.iterations == 0 || config.iterations > LIMIT {
        return Err("invalid_iteration_cap".into());
    }
    let total = Instant::now();
    let setup = Instant::now();
    let mut row = empty();
    let chunked = arm.ends_with("-c14") || arm.ends_with("-c1") || arm == "full-chunked";
    let persistent = arm.contains("-p") || chunked;
    let forest = arm.starts_with("forest");
    let edges = if forest { p.edge_bound() } else { 0 };
    let base = memory_base(p, rhs, forest, edges)?;
    let transient = if forest {
        edges
            .checked_mul(57)
            .and_then(|v| v.checked_add(p.n.checked_mul(520)?))
            .and_then(|v| v.checked_add(65536))
            .ok_or("memory_overflow")?
    } else {
        0
    };
    if base.checked_add(transient).ok_or("memory_overflow")? > config.memory {
        return Err("baseline_memory_refused".into());
    }
    row.solutions = Vec::with_capacity(rhs.len());
    row.decisions = Vec::with_capacity(rhs.len());
    let graph = if forest {
        let t = Instant::now();
        let g = Arc::new(p.graph());
        row.graph_seconds = t.elapsed().as_secs_f64();
        row.graph_builds = 1;
        Some(g)
    } else {
        None
    };
    let mut cheap = if forest {
        Preconditioner::Forest(Forest::new(graph.as_ref().unwrap()))
    } else {
        Preconditioner::Diagonal(p.diagonal.clone())
    };
    let cheap_tariff = Tariff::new(
        p,
        if forest {
            24 * p.n as u64
        } else {
            8 * p.n as u64
        },
    );
    row.setup += setup.elapsed().as_secs_f64();
    std::thread::scope(|scope| -> Result<(), String> {
        let mut ready: Option<Ready> = None;
        let mut ledger = Ledger {
            denominator: config.denominator,
            ..Ledger::default()
        };
        let mut disabled = false;
        let mut retired_build = 0u64;
        let mut task_number = 0u64;
        let mut admitted_peak = base + transient;
        if arm.starts_with("full-") {
            let t = Instant::now();
            let mut task = BuildTask::start(scope, p, None, base, config.memory, (0, chunked))?;
            task.grant(CAP)?;
            if !task.terminal {
                return Err("full_controlled_budget_exhausted".into());
            }
            row.graph_builds += task.report.graph_builds;
            row.graph_seconds += task.report.graph_seconds;
            ready = task.ready.take();
            if ready.is_none() {
                return Err("full_controlled_memory_refused".into());
            }
            row.setup += t.elapsed().as_secs_f64();
            row.tail_levels = ready.as_ref().unwrap().levels;
        }
        let mut task: Option<BuildTask<'_>> = None;
        for (rhs_index, b) in rhs.iter().enumerate() {
            if arm.starts_with("full-") {
                let full = ready.as_mut().unwrap();
                let tariff = Tariff::new(p, full.cycle);
                let mut state = PcgState::new(p, b, &mut full.pre, tariff, None)?;
                while !state.done {
                    if state.iterations >= config.iterations {
                        return Err("iteration_limit".into());
                    }
                    state.advance(
                        p,
                        b,
                        &mut full.pre,
                        state.iterations + 1 == config.iterations,
                    )?;
                }
                state_counts(&mut row, &state);
                publish(&mut row, state);
                continue;
            }
            let mut inc = PcgState::new(p, b, &mut cheap, cheap_tariff, None)?;
            ledger.inc = ledger.inc.checked_add(inc.work).ok_or("work_overflow")?;
            let mut heavy: Option<PcgState> = None;
            let resumed = task.is_some();
            let start_issued = task.as_ref().map_or(0, |t| t.report.snapshot.issued);
            let start_charged = task.as_ref().map_or(0, |t| t.report.snapshot.charged);
            let start_boundaries = task.as_ref().map_or(0, |t| t.report.boundaries);
            let mut shared = 0usize;
            let mut winner = "incumbent";
            let mut build_status = if ready.is_some() {
                "reused"
            } else if resumed {
                "parked"
            } else if disabled {
                "disabled"
            } else {
                "not_started"
            };
            let mut build_trace = task.as_ref().map_or(0, |t| t.report.trace);
            let mut peak = admitted_peak;
            let mut grants = 0usize;
            let mut install = None;
            let mut built_now = false;
            let mut boundaries = start_boundaries;
            let mut build_active = task.as_ref().map_or(0., |t| t.report.active_seconds);
            let mut end_issued = start_issued;
            let mut end_charged = start_charged;
            while !inc.done {
                if shared >= config.iterations {
                    return Err(format!(
                        "iteration_limit_incumbent_{}_challenger_{}",
                        inc.iterations,
                        heavy.as_ref().map_or(0, |s| s.iterations)
                    ));
                }
                let before = inc.work;
                if let Err(error) = inc.advance(p, b, &mut cheap, shared + 1 == config.iterations) {
                    return Err(if persistent {
                        format!(
                            "rhs_{rhs_index}_incumbent_{}",
                            inc.failure_context(p, b, error)
                        )
                    } else {
                        error
                    });
                }
                ledger.inc = ledger
                    .inc
                    .checked_add(inc.work - before)
                    .ok_or("work_overflow")?;
                shared += 1;
                if inc.done {
                    break;
                }
                if shared == config.iterations {
                    return Err(format!(
                        "iteration_limit_incumbent_{}_challenger_{}",
                        inc.iterations,
                        heavy.as_ref().map_or(0, |s| s.iterations)
                    ));
                }
                if inc.iterations % 8 != 0 {
                    continue;
                }
                // Fixed nominal control allowance includes admission, event records and final join.
                if !ledger.spend(256, 2) {
                    continue;
                }
                if ready.is_none()
                    && !disabled
                    && base
                        .checked_add(crate::builder::STACK + 65536)
                        .ok_or("memory_overflow")?
                        > config.memory
                {
                    disabled = true;
                    build_status = "refused_before_spawn";
                }
                if ready.is_none() && !disabled {
                    if task.is_none() && ledger.spend(4096, 2) {
                        let t = Instant::now();
                        task_number += 1;
                        task = Some(BuildTask::start(
                            scope,
                            p,
                            graph.clone(),
                            base,
                            config.memory,
                            (task_number, chunked),
                        )?);
                        row.setup += t.elapsed().as_secs_f64();
                        build_status = "parked";
                    }
                    if let Some(build) = task.as_mut() {
                        let grant = ledger
                            .available()
                            .min(CAP - retired_build - build.report.snapshot.issued);
                        if grant > 0 {
                            assert!(ledger.spend(grant, 0));
                            let t = Instant::now();
                            build.grant(build.report.snapshot.issued + grant)?;
                            row.setup += t.elapsed().as_secs_f64();
                            grants += 1;
                        }
                        end_issued = build.report.snapshot.issued;
                        end_charged = build.report.snapshot.charged;
                        boundaries = build.report.boundaries;
                        build_active = build.report.active_seconds;
                        build_trace = build.report.trace;
                        peak = peak.max(build.report.memory);
                        admitted_peak = admitted_peak.max(peak);
                        if build.terminal {
                            retire(build, &mut ledger, &mut row)?;
                            retired_build += build.report.snapshot.charged;
                            ready = build.ready.take();
                            built_now = ready.is_some();
                            disabled = ready.is_none();
                            build_status = if disabled { "refused" } else { "complete" };
                            if let Some(r) = &ready {
                                row.upgrades += 1;
                                row.tail_levels = r.levels;
                            }
                            task = None;
                        }
                    }
                }
                if let Some(full) = ready.as_mut() {
                    let tariff = Tariff::new(p, full.cycle);
                    if heavy.is_none() && ledger.spend(tariff.init_bound(), 1) {
                        let s = PcgState::new(
                            p,
                            b,
                            &mut full.pre,
                            tariff,
                            if built_now { Some(&inc.x) } else { None },
                        )?;
                        assert!(s.work <= tariff.init_bound());
                        heavy = Some(s);
                        install = Some(inc.iterations);
                    }
                    if let Some(s) = heavy.as_mut() {
                        while !s.done
                            && shared < config.iterations
                            && ledger.spend(tariff.step_bound(), 1)
                        {
                            let before = s.work;
                            if let Err(error) =
                                s.advance(p, b, &mut full.pre, shared + 1 == config.iterations)
                            {
                                return Err(if persistent {
                                    format!(
                                        "rhs_{rhs_index}_challenger_{}",
                                        s.failure_context(p, b, error)
                                    )
                                } else {
                                    error
                                });
                            }
                            assert!(s.work - before <= tariff.step_bound());
                            shared += 1;
                        }
                        if s.done {
                            winner = "challenger";
                            break;
                        }
                    }
                }
            }
            if (!persistent || rhs_index + 1 == rhs.len()) && task.is_some() {
                let build = task.as_mut().unwrap();
                retire(build, &mut ledger, &mut row)?;
                end_issued = build.report.snapshot.issued;
                end_charged = build.report.snapshot.charged;
                retired_build += build.report.snapshot.charged;
                boundaries = build.report.boundaries;
                build_active = build.report.active_seconds;
                build_trace = build.report.trace;
                peak = peak.max(build.report.memory);
                admitted_peak = admitted_peak.max(peak);
                build_status = "abandoned";
                task = None;
            }
            assert!(ledger.optional <= ledger.allowance());
            let mut decision = String::with_capacity(4096);
            write!(&mut decision,"{{\"rhs\":{rhs_index},\"winner\":\"{winner}\",\"incumbent_iterations\":{},\"challenger_iterations\":{},\"incumbent_work\":{},\"optional_work\":{},\"build_work\":{},\"challenger_work\":{},\"control_work\":{},\"share_denominator\":{},\"build_status\":\"{build_status}\",\"build_trace\":\"{build_trace:016x}\",\"incumbent_trace\":\"{:016x}\",\"memory_bound\":{peak},\"grants\":{grants},\"build_boundaries\":{boundaries},\"build_active_seconds\":{build_active:.9},\"install_iteration\":{}}}",inc.iterations,heavy.as_ref().map_or(0,|s|s.iterations),ledger.inc,ledger.optional,ledger.build,ledger.challenger,ledger.control,config.denominator,inc.trace,install.map_or("null".into(),|v|v.to_string())).unwrap();
            if persistent {
                decision.pop();
                let reserved = task
                    .as_ref()
                    .map_or(0, |t| t.report.snapshot.issued - t.report.snapshot.charged);
                let pending_phase = task
                    .as_ref()
                    .and_then(|t| t.report.snapshot.pending)
                    .map_or("", |b| b.boundary.phase);
                write!(&mut decision,",\"task_id\":{task_number},\"resumed\":{resumed},\"issued_start\":{start_issued},\"charged_start\":{start_charged},\"issued_end\":{end_issued},\"charged_end\":{end_charged},\"boundaries_start\":{start_boundaries},\"reserved_build_work\":{reserved},\"pending_phase\":\"{pending_phase}\"}}").unwrap();
            }
            assert!(decision.len() <= 4096);
            row.decisions.push(decision);
            state_counts(&mut row, &inc);
            if let Some(s) = &heavy {
                state_counts(&mut row, s);
            }
            if winner == "challenger" {
                publish(&mut row, heavy.unwrap());
            } else {
                publish(&mut row, inc);
            }
        }
        Ok(())
    })?;
    row.seconds = total.elapsed().as_secs_f64();
    Ok(row)
}
