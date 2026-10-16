//! A stackful serial builder. Only the constructing thread touches BuildPrefix.
use crate::preconditioner::{Full, Preconditioner};
use crate::problem::Problem;
use cmg::{
    BuildBoundary, BuildControl, BuildDecision, BuildOperation, BuildPrefix, CmgOptions,
    CmgPreconditioner, ComponentBuildOptions, Laplacian, PrefixDecision, PrefixOutcome,
    PrefixSnapshot,
};
use std::sync::{mpsc, Arc};
use std::thread::{Scope, ScopedJoinHandle};
use std::time::Instant;

pub const CAP: u64 = 1_000_000_000_000;
pub const STACK: usize = 8 * 1024 * 1024;
pub struct Ready {
    pub pre: Preconditioner,
    pub cycle: u64,
    pub levels: usize,
}
pub struct Report {
    pub snapshot: PrefixSnapshot,
    pub trace: u64,
    pub boundaries: u64,
    pub memory: usize,
    pub graph_builds: usize,
    pub graph_seconds: f64,
    pub active_seconds: f64,
}
enum Command {
    Grant(u64),
    Abandon,
}
enum Event {
    Parked(Report),
    Finished(Report, Result<Option<Ready>, String>),
}
struct Control {
    prefix: BuildPrefix,
    dense_chunks: bool,
    rx: mpsc::Receiver<Command>,
    tx: mpsc::Sender<Event>,
    trace: u64,
    boundaries: u64,
    memory: usize,
    graph_builds: usize,
    graph_seconds: f64,
    active: f64,
    clock: Instant,
}
impl Control {
    fn report(&self) -> Report {
        Report {
            snapshot: self.prefix.snapshot(),
            trace: self.trace,
            boundaries: self.boundaries,
            memory: self.memory,
            graph_builds: self.graph_builds,
            graph_seconds: self.graph_seconds,
            active_seconds: self.active + self.clock.elapsed().as_secs_f64(),
        }
    }
    fn work(&mut self, phase: &'static str, units: u16) -> Result<(), String> {
        self.gate(BuildBoundary {
            phase,
            chunk_id: self.boundaries,
            charged_work: 0,
            operation: BuildOperation::Work { units },
        })
    }
    fn gate(&mut self, b: BuildBoundary) -> Result<(), String> {
        if self.checkpoint(b) == BuildDecision::Continue {
            Ok(())
        } else {
            Err("builder_stopped".into())
        }
    }
}
impl Control {
    fn admit(&mut self, b: BuildBoundary) -> BuildDecision {
        loop {
            match self.prefix.admit(b).expect("valid prefix transition") {
                PrefixDecision::Continue => {
                    self.boundaries += 1;
                    let value = match b.operation {
                        BuildOperation::Work { units } => u64::from(units),
                        BuildOperation::Allocation { bytes } => bytes as u64 ^ 0x8000000000000000,
                    };
                    self.trace = (self.trace ^ value).wrapping_mul(1099511628211);
                    return BuildDecision::Continue;
                }
                PrefixDecision::Park => {
                    self.prefix.drain_issued_prefix().unwrap();
                    assert!(self.prefix.snapshot().drained());
                    self.active += self.clock.elapsed().as_secs_f64();
                    self.clock = Instant::now();
                    if self.tx.send(Event::Parked(self.report())).is_err() {
                        self.prefix.interrupt();
                        return BuildDecision::Interrupted;
                    }
                    let command = self.rx.recv();
                    self.clock = Instant::now();
                    match command {
                        Ok(Command::Grant(g)) => {
                            self.prefix.grant(g).unwrap();
                            self.prefix.drain_issued_prefix().unwrap();
                        }
                        _ => {
                            self.prefix.abandon_unissued_suffix().unwrap();
                        }
                    }
                }
                PrefixDecision::StopUnused => return BuildDecision::StopUnused,
                PrefixDecision::Interrupted => return BuildDecision::Interrupted,
            }
        }
    }
}
impl BuildControl for Control {
    fn group_dense_work(&self) -> bool {
        self.dense_chunks
    }
    fn checkpoint(&mut self, b: BuildBoundary) -> BuildDecision {
        // Price callback/trace bookkeeping as a separate ordered work gate.
        let overhead = BuildBoundary {
            phase: "callback bookkeeping",
            chunk_id: self.boundaries,
            charged_work: 0,
            operation: BuildOperation::Work { units: 32 },
        };
        let decision = self.admit(overhead);
        if decision != BuildDecision::Continue {
            return decision;
        }
        self.admit(b)
    }
}

fn construct(
    p: &Problem,
    graph: Option<Arc<Laplacian>>,
    base: usize,
    memory_limit: usize,
    c: &mut Control,
) -> Result<Ready, String> {
    c.work("wrapper admission", 1024)?;
    let mut count = 0usize;
    if let Some(g) = &graph {
        count = g.edge_count();
    } else {
        for e in p.offsets.windows(2) {
            c.work("clique count", 16)?;
            let d = e[1] - e[0];
            count = count
                .checked_add(d.checked_mul(d.saturating_sub(1)).ok_or("edge_overflow")? / 2)
                .ok_or("edge_overflow")?;
        }
    }
    let options = CmgOptions::default();
    let experiment = ComponentBuildOptions::default();
    let construction = CmgPreconditioner::component_serial_construction_allocation_bound(
        p.n, count, options, experiment,
    )
    .map_err(|e| e.to_string())?;
    let workspace = CmgPreconditioner::scalar_workspace_allocation_bound(p.n, options)
        .map_err(|e| e.to_string())?;
    // 24-byte tuple + two 16-byte compact edge banks, diagonal and Arc owners.
    let graph_bytes = if graph.is_none() {
        count
            .checked_mul(56)
            .and_then(|b| b.checked_add(p.n.checked_mul(8)?))
            .and_then(|b| b.checked_add(128))
            .ok_or("memory_overflow")?
    } else {
        0
    };
    let requested = [base, construction, workspace, graph_bytes, STACK, 65536]
        .into_iter()
        .try_fold(0usize, |a, b| a.checked_add(b))
        .ok_or("memory_overflow")?;
    if requested > memory_limit {
        return Err("memory_refused".into());
    }
    c.memory = requested;
    let graph = if let Some(g) = graph {
        g
    } else {
        let before = c.active + c.clock.elapsed().as_secs_f64(); // graph seconds below excludes parks using active clock.
        let mut edges = Vec::new();
        c.gate(BuildBoundary {
            phase: "clique tuple allocation",
            chunk_id: 0,
            charged_work: 0,
            operation: BuildOperation::Allocation { bytes: count * 24 },
        })?;
        edges
            .try_reserve_exact(count)
            .map_err(|_| "tuple_allocation_failed")?;
        for (w, e) in p.offsets.windows(2).enumerate() {
            c.work("clique worker", 16)?;
            for i in e[0]..e[1] {
                c.work("clique row", 8)?;
                for j in i + 1..e[1] {
                    c.work("clique edge", 16)?;
                    edges.push((
                        p.firms[i],
                        p.firms[j],
                        p.weights[i] * (p.weights[j] / p.totals[w]),
                    ));
                }
            }
        }
        let g = Laplacian::from_edges_controlled(p.n, &edges, graph_bytes, CAP, c)
            .map_err(|e| e.to_string())?;
        drop(edges);
        c.graph_builds = 1;
        // Exact elapsed active time is tracked by callback pauses; total setup is authoritative.
        c.graph_seconds = c.active + c.clock.elapsed().as_secs_f64() - before;
        Arc::new(g)
    };
    let pre = CmgPreconditioner::build_component_experiment_serial_controlled(
        &graph,
        options,
        experiment,
        construction,
        CAP,
        c,
    )
    .map_err(|e| e.to_string())?;
    let exact = pre
        .workspace_allocation_bytes_controlled(CAP, c)
        .map_err(|e| e.to_string())?;
    if exact > workspace {
        return Err("workspace_bound_failed".into());
    }
    let ws = pre
        .try_workspace_controlled(CAP, c)
        .map_err(|e| e.to_string())?;
    let cycle = pre
        .scalar_cycle_work_bound_controlled(CAP, c)
        .map_err(|e| e.to_string())?;
    let levels = pre.hierarchy().levels().len();
    Ok(Ready {
        pre: Preconditioner::Full(Box::new(Full::from_parts(pre, ws))),
        cycle,
        levels,
    })
}
pub struct BuildTask<'s> {
    tx: mpsc::Sender<Command>,
    rx: mpsc::Receiver<Event>,
    handle: Option<ScopedJoinHandle<'s, ()>>,
    pub report: Report,
    pub ready: Option<Ready>,
    pub terminal: bool,
    pub refusal: bool,
}
impl<'s> BuildTask<'s> {
    pub fn start<'e>(
        scope: &'s Scope<'s, 'e>,
        p: &'e Problem,
        graph: Option<Arc<Laplacian>>,
        base: usize,
        memory_limit: usize,
        (task, dense_chunks): (u64, bool),
    ) -> Result<Self, String> {
        if base.checked_add(STACK + 65536).ok_or("memory_overflow")? > memory_limit {
            return Err("memory_refused_before_spawn".into());
        }
        let (tx, commands) = mpsc::channel();
        let (events, rx) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .stack_size(STACK)
            .spawn_scoped(scope, move || {
                let mut c = Control {
                    prefix: BuildPrefix::new(task, CAP),
                    dense_chunks,
                    rx: commands,
                    tx: events,
                    trace: 14695981039346656037,
                    boundaries: 0,
                    memory: base + STACK + 65536,
                    graph_builds: 0,
                    graph_seconds: 0.,
                    active: 0.,
                    clock: Instant::now(),
                };
                let result = construct(p, graph, base, memory_limit, &mut c);
                let outcome = c.prefix.snapshot().outcome.unwrap_or(match &result {
                    Ok(_) => PrefixOutcome::Complete,
                    Err(e) if e == "memory_refused" => PrefixOutcome::Unused,
                    Err(_) => PrefixOutcome::Fatal { kind: 1 },
                });
                c.prefix.finish(outcome).unwrap();
                let result = match outcome {
                    PrefixOutcome::Unused | PrefixOutcome::BudgetExhausted => Ok(None),
                    _ => result.map(Some),
                };
                let _ = c.tx.send(Event::Finished(c.report(), result));
            })
            .map_err(|e| e.to_string())?;
        let initial = BuildPrefix::new(task, CAP).snapshot();
        let mut s = Self {
            tx,
            rx,
            handle: Some(handle),
            report: Report {
                snapshot: initial,
                trace: 0,
                boundaries: 0,
                memory: base,
                graph_builds: 0,
                graph_seconds: 0.,
                active_seconds: 0.,
            },
            ready: None,
            terminal: false,
            refusal: false,
        };
        s.receive()?;
        Ok(s)
    }
    fn receive(&mut self) -> Result<(), String> {
        match self
            .rx
            .recv()
            .map_err(|_| "builder_panicked_or_disconnected")?
        {
            Event::Parked(r) => {
                assert!(r.snapshot.drained());
                self.report = r;
            }
            Event::Finished(r, result) => {
                self.report = r;
                self.terminal = true;
                self.handle
                    .take()
                    .unwrap()
                    .join()
                    .map_err(|_| "builder_panicked")?;
                self.ready = result?;
                self.refusal = self.ready.is_none();
            }
        }
        Ok(())
    }
    pub fn grant(&mut self, g: u64) -> Result<(), String> {
        assert!(!self.terminal);
        self.tx
            .send(Command::Grant(g))
            .map_err(|_| "builder_disconnected")?;
        self.receive()
    }
    pub fn abandon(&mut self) -> Result<(), String> {
        if !self.terminal {
            self.tx
                .send(Command::Abandon)
                .map_err(|_| "builder_disconnected")?;
            self.receive()?;
        }
        Ok(())
    }
}
impl Drop for BuildTask<'_> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = self.tx.send(Command::Abandon);
            let _ = handle.join();
        }
    }
}
