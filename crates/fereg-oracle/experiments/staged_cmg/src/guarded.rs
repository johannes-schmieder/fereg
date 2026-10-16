//! Synchronous admitted CMG construction: cancellation, without speculative scheduling.
use crate::preconditioner::Full;
use crate::problem::Problem;
use cmg::{
    BuildBoundary, BuildControl, BuildDecision, CmgError, CmgOptions, CmgPreconditioner,
    ComponentBuildOptions, Laplacian,
};
use std::fmt;
use std::time::Instant;

pub const MEMORY: usize = 1 << 30;
pub const DIAGNOSTIC_MEMORY: usize = 8 << 30;

#[derive(Clone, Copy, Default)]
pub enum Constructor {
    #[default]
    Controlled,
    /// Diagnostic only: ordinary admitted kernels have no internal cancellation.
    Admitted,
}
const WORK: u64 = 1_000_000_000_000;

#[derive(Debug)]
pub enum Error {
    Memory { required: usize, limit: usize },
    Fatal(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Memory { required, limit } => {
                write!(f, "memory_refused_required_{required}_limit_{limit}")
            }
            Self::Fatal(reason) => f.write_str(reason),
        }
    }
}
impl From<CmgError> for Error {
    fn from(e: CmgError) -> Self {
        match e {
            CmgError::MemoryBudgetExceeded {
                required_bytes,
                budget_bytes,
            } => Self::Memory {
                required: required_bytes,
                limit: budget_bytes,
            },
            e => Self::Fatal(e.to_string()),
        }
    }
}
fn overflow() -> Error {
    Error::Fatal("memory_overflow".into())
}
pub fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or_else(overflow)
}
pub fn bytes(n: usize, width: usize) -> Result<usize, Error> {
    n.checked_mul(width).ok_or_else(overflow)
}
pub fn admit(required: usize, limit: usize) -> Result<(), Error> {
    if required > limit {
        Err(Error::Memory { required, limit })
    } else {
        Ok(())
    }
}
pub fn edges<F: FnMut(&'static str) -> bool>(
    p: &Problem,
    c: &mut Control<F>,
) -> Result<usize, Error> {
    let mut sum = 0;
    for (i, e) in p.offsets.windows(2).enumerate() {
        if i % 64 == 0 {
            c.check("clique count")?;
        }
        let d = e[1] - e[0];
        sum = add(sum, bytes(d, d.saturating_sub(1))? / 2)?;
    }
    Ok(sum)
}
/// Requested-storage model, not RSS: all input capacities, RHS/output banks,
/// recurrence vectors, cheap preconditioner, projection scratch and telemetry.
pub fn base(p: &Problem, rhs: &[Vec<f64>]) -> Result<usize, Error> {
    let mut total = 65536;
    for capacity in [
        p.offsets.capacity(),
        p.firms.capacity(),
        p.weights.capacity(),
        p.totals.capacity(),
        p.diagonal.capacity(),
        p.labels.capacity(),
        p.sizes.capacity(),
    ] {
        total = add(total, bytes(capacity, 8)?)?;
    }
    for b in rhs {
        total = add(total, bytes(b.capacity(), 8)?)?;
    }
    total = add(total, bytes(p.n, bytes(add(rhs.len(), 16)?, 8)?)?)?;
    add(total, bytes(rhs.len(), 16384)?)
}
pub fn graph_bound(p: &Problem, count: usize) -> Result<usize, Error> {
    // Input tuples plus both compact banks, diagonal and Arc owners.
    add(add(bytes(count, 56)?, bytes(p.n, 8)?)?, 256)
}
pub struct Control<F> {
    pub cancel: F,
}
impl<F: FnMut(&'static str) -> bool> Control<F> {
    pub fn check(&mut self, phase: &'static str) -> Result<(), Error> {
        if (self.cancel)(phase) {
            Err(Error::Fatal("interrupted".into()))
        } else {
            Ok(())
        }
    }
}
impl<F: FnMut(&'static str) -> bool> BuildControl for Control<F> {
    fn cooperative_work_batch(&self) -> u16 {
        u16::MAX
    }
    fn group_dense_work(&self) -> bool {
        true
    }
    fn checkpoint(&mut self, b: BuildBoundary) -> BuildDecision {
        if (self.cancel)(b.phase) {
            BuildDecision::Interrupted
        } else {
            BuildDecision::Continue
        }
    }
}
pub fn graph<F: FnMut(&'static str) -> bool>(
    p: &Problem,
    count: usize,
    c: &mut Control<F>,
) -> Result<Laplacian, Error> {
    c.check("clique allocation")?;
    let mut tuples = Vec::new();
    tuples
        .try_reserve_exact(count)
        .map_err(|_| Error::Fatal("tuple_allocation_failed".into()))?;
    for (w, e) in p.offsets.windows(2).enumerate() {
        c.check("clique worker")?;
        for i in e[0]..e[1] {
            for j in i + 1..e[1] {
                if tuples.len() % 64 == 0 {
                    c.check("clique edges")?;
                }
                tuples.push((
                    p.firms[i],
                    p.firms[j],
                    p.weights[i] * (p.weights[j] / p.totals[w]),
                ));
            }
        }
    }
    Ok(Laplacian::from_edges_controlled(
        p.n,
        &tuples,
        graph_bound(p, count)?,
        WORK,
        c,
    )?)
}

/// Fallible initialized storage with cancellation between bounded copy chunks.
pub fn filled<T: Copy, F: FnMut(&'static str) -> bool>(
    n: usize,
    value: T,
    c: &mut Control<F>,
) -> Result<Vec<T>, Error> {
    c.check("wrapper allocation")?;
    let mut out = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| Error::Fatal("wrapper_allocation_failed".into()))?;
    while out.len() < n {
        c.check("wrapper initialization")?;
        out.resize((out.len() + 64).min(n), value);
    }
    Ok(out)
}

pub struct Built {
    pub full: Full,
    pub peak: usize,
    pub graph_seconds: f64,
    pub graph_builds: usize,
}
pub fn build<F: FnMut(&'static str) -> bool>(
    p: &Problem,
    existing: Option<&Laplacian>,
    base: usize,
    limit: usize,
    c: &mut Control<F>,
) -> Result<Built, Error> {
    build_with(p, existing, base, limit, Constructor::Controlled, c)
}
pub fn build_with<F: FnMut(&'static str) -> bool>(
    p: &Problem,
    existing: Option<&Laplacian>,
    base: usize,
    limit: usize,
    constructor: Constructor,
    c: &mut Control<F>,
) -> Result<Built, Error> {
    c.check("full admission")?;
    let count = existing.map_or_else(|| edges(p, c), |g| Ok(g.edge_count()))?;
    let options = CmgOptions::default();
    let experiment = ComponentBuildOptions::default();
    let construction = CmgPreconditioner::component_serial_construction_allocation_bound(
        p.n, count, options, experiment,
    )?;
    let workspace = CmgPreconditioner::scalar_workspace_allocation_bound(p.n, options)?;
    let graph_bytes = if existing.is_some() {
        0
    } else {
        graph_bound(p, count)?
    };
    let peak = add(add(add(base, construction)?, workspace)?, graph_bytes)?;
    admit(peak, limit)?;
    let clock = Instant::now();
    let owned = if existing.is_none() {
        Some(graph(p, count, c)?)
    } else {
        None
    };
    let graph_seconds = clock.elapsed().as_secs_f64();
    let g = existing
        .or(owned.as_ref())
        .expect("existing or constructed graph");
    let (pre, ws) = match constructor {
        Constructor::Controlled => {
            let pre = CmgPreconditioner::build_component_experiment_serial_controlled(
                g,
                options,
                experiment,
                construction,
                WORK,
                c,
            )?;
            let exact = pre.workspace_allocation_bytes_controlled(WORK, c)?;
            if exact > workspace {
                return Err(Error::Fatal("workspace_bound_failed".into()));
            }
            let ws = pre.try_workspace_controlled(WORK, c)?;
            (pre, ws)
        }
        Constructor::Admitted => {
            // Benchmark reference only. The whole hierarchy/factor call is uncancellable.
            let pre = CmgPreconditioner::build_component_experiment_serial_with_budget(
                g,
                options,
                experiment,
                construction,
            )?;
            if pre.workspace_allocation_bytes()? > workspace {
                return Err(Error::Fatal("workspace_bound_failed".into()));
            }
            let ws = pre.try_workspace()?;
            (pre, ws)
        }
    };
    c.check("full publication")?;
    Ok(Built {
        full: Full::from_parts(pre, ws),
        peak,
        graph_seconds,
        graph_builds: usize::from(owned.is_some()),
    })
}
