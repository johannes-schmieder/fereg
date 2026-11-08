//! Combinatorial multigrid (CMG) local solves on one retained block's
//! Laplacian `S_bb = Σ_s (diag(a_s) - a_s a_s' / W_s)`.
//!
//! The CMG graph has the block's connected levels as its first vertices (in
//! ascending level order), followed by one auxiliary vertex per segment with
//! more than [`STAR_LEVELS`] distinct levels. Such a segment contributes the
//! star edges `a_sj` between its auxiliary vertex and its levels; eliminating
//! the auxiliary vertex (degree `W_s`) leaves exactly the segment's clique.
//! Smaller segments contribute their clique edges `a_sj a_sk / W_s` directly,
//! unless a clique weight would leave the normal range, in which case the
//! segment also becomes a star. Duplicate edges are aggregated by CMG's
//! Laplacian builder. If `L` is the augmented Laplacian, the restriction of
//! `L^+ [r; 0]` to the level vertices is `S_bb^+ r` up to component constants,
//! so one fixed CMG cycle `B ≈ L^+` gives the local solve `P R B E P`: `E`
//! pads auxiliary vertices with zeros, `R` restricts to levels and `P`
//! removes each connected component's mean. It is symmetric positive
//! semidefinite because the cycle is, and fixed because the hierarchy is.
//! Levels in no segment with two or more levels are isolated: their rows of
//! `S_bb` vanish and their image is zero.
//!
//! Application is serial: CMG's parallel cycle needs a borrowed
//! `rayon::ThreadPool` (a `cmg::Team`) and a row-operator plan built for it,
//! whereas projections run on the caller's current pool without a handle.
//!
//! Memory: every allocation is fallible. The graph's raw edges are released
//! once the Laplacian is built; the hierarchy, component labels and the
//! application buffers are retained. Buffers grow once to the widest
//! application (at most [`MAX_COLUMNS`] lanes per CMG call), so repeated
//! applications allocate nothing.

// Stage 3: the block-Jacobi driver installs these solves.
#![allow(dead_code)]

use crate::elimination::block::{BlockLaplacian, LocalSolve};
use crate::error::{Error, Result};
use cmg::{
    ApplyContext, BuildContext, BuildOptions, CmgBatchWorkspace, CmgError, CmgOptions,
    CmgPreconditioner, ComponentBuildOptions, Laplacian,
};

/// Segments with more distinct levels than this become auxiliary stars.
const STAR_LEVELS: usize = 5;
/// Lanes per CMG batch application; wider applications are split.
const MAX_COLUMNS: usize = 4;
/// Graph vertex of an isolated level.
const ISOLATED: u32 = u32::MAX;
/// Segments between two cancellation polls of the graph passes.
const POLL_SEGMENTS: usize = 1 << 12;

/// One fixed CMG approximation of a block Laplacian's pseudo-inverse.
pub(crate) struct CmgLocal {
    /// Graph vertex of each level ([`ISOLATED`] when isolated); connected
    /// levels take the vertices `0..connected` in ascending level order.
    vertex: Vec<u32>,
    /// Component of each level vertex.
    component: Vec<u32>,
    /// Level vertices of each component.
    sizes: Vec<u32>,
    /// Level vertices followed by auxiliary vertices.
    vertices: usize,
    preconditioner: CmgPreconditioner,
    /// Lanes the buffers and the CMG workspace hold (0 before the first
    /// application).
    columns: usize,
    workspace: Option<CmgBatchWorkspace>,
    workspace_bytes: usize,
    rhs: Vec<f64>,
    solution: Vec<f64>,
    /// Compensated per-component sums of one batch.
    sums: Vec<f64>,
    corrections: Vec<f64>,
}

impl CmgLocal {
    /// Build the hierarchy of `block`. `None` when the block has fewer than
    /// two connected levels, or when its graph cannot be represented with
    /// positive normal weights (the caller keeps another local solve).
    /// `cancel` is polled between bounded graph passes and inside CMG.
    pub(crate) fn build(
        block: &BlockLaplacian,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<Option<Self>> {
        validate(block)?;
        let levels = block.levels;
        let segments = block.totals.len();
        let poll = |segment: usize| -> Result<()> {
            if segment % POLL_SEGMENTS == 0 && cancel() {
                Err(Error::Cancelled)
            } else {
                Ok(())
            }
        };
        // Connected levels: those in a segment with at least two entries.
        let mut vertex = filled(levels, ISOLATED)?;
        for s in 0..segments {
            poll(s)?;
            let cells = segment(block, s);
            if cells.len() >= 2 {
                for &(level, _) in cells {
                    vertex[level as usize] = 0;
                }
            }
        }
        let mut connected = 0_u32;
        for v in &mut vertex {
            if *v != ISOLATED {
                *v = connected;
                connected += 1;
            }
        }
        if connected < 2 {
            return Ok(None);
        }
        let (component, sizes) = components(block, &vertex, connected as usize, &poll)?;
        // Count the graph, then emit exactly that many edges.
        let mut edges_needed = 0_usize;
        let mut stars = 0_usize;
        for s in 0..segments {
            poll(s)?;
            let cells = segment(block, s);
            match shape(cells, block.totals[s]) {
                Shape::Single => {}
                Shape::Clique => edges_needed += cells.len() * (cells.len() - 1) / 2,
                Shape::Star => {
                    edges_needed += cells.len();
                    stars += 1;
                }
            }
        }
        let vertices = connected as usize + stars;
        if vertices >= ISOLATED as usize {
            return Ok(None);
        }
        let mut edges: Vec<(usize, usize, f64)> = with_capacity(edges_needed)?;
        let mut auxiliary = connected as usize;
        for s in 0..segments {
            poll(s)?;
            let cells = segment(block, s);
            let total = block.totals[s];
            match shape(cells, total) {
                Shape::Single => {}
                Shape::Clique => {
                    for (i, &(left, a)) in cells.iter().enumerate() {
                        for &(right, b) in &cells[i + 1..] {
                            if left != right {
                                let weight = a.min(b) * (a.max(b) / total);
                                edges.push((
                                    vertex[left as usize] as usize,
                                    vertex[right as usize] as usize,
                                    weight,
                                ));
                            }
                        }
                    }
                }
                Shape::Star => {
                    for &(level, a) in cells {
                        edges.push((vertex[level as usize] as usize, auxiliary, a));
                    }
                    auxiliary += 1;
                }
            }
        }
        let context = BuildContext {
            cancel: Some(cancel),
            ..BuildContext::default()
        };
        let graph = match Laplacian::build(vertices, &edges, context) {
            Ok(graph) => graph,
            Err(error) => return built(error),
        };
        drop(edges);
        let options = BuildOptions {
            hierarchy: CmgOptions {
                // Reduced Laplacians of worker-firm panels are expander-like:
                // the low-effective-degree correction would detach most
                // vertices into singleton aggregates.
                low_effective_degree_threshold: 0.0,
                ..CmgOptions::default()
            },
            components: ComponentBuildOptions {
                prune_coarse_isolates: true,
                preserve_unpruned_stopping: true,
                factor_terminal_components: true,
            },
        };
        let preconditioner = match CmgPreconditioner::build_with(&graph, options, context) {
            Ok(preconditioner) => preconditioner,
            Err(error) => return built(error),
        };
        drop(graph);
        Ok(Some(Self {
            vertex,
            component,
            sizes,
            vertices,
            preconditioner,
            columns: 0,
            workspace: None,
            workspace_bytes: 0,
            rhs: Vec::new(),
            solution: Vec::new(),
            sums: Vec::new(),
            corrections: Vec::new(),
        }))
    }

    /// Hold buffers for applications of up to `width` lanes per CMG call
    /// (capped at [`MAX_COLUMNS`]). Growth happens once per wider width.
    pub(crate) fn reserve(&mut self, width: usize) -> Result<()> {
        let columns = width.clamp(1, MAX_COLUMNS);
        if columns <= self.columns {
            return Ok(());
        }
        self.workspace = None;
        self.columns = 0;
        let message = "CMG local solve buffers overflow";
        let values = self
            .vertices
            .checked_mul(columns)
            .ok_or(Error::OutOfMemory(message))?;
        let sums = self
            .sizes
            .len()
            .checked_mul(columns)
            .ok_or(Error::OutOfMemory(message))?;
        refill(&mut self.rhs, values)?;
        refill(&mut self.solution, values)?;
        refill(&mut self.sums, sums)?;
        refill(&mut self.corrections, sums)?;
        let pre = &self.preconditioner;
        self.workspace_bytes = CmgBatchWorkspace::allocation_bytes(pre, columns).map_err(fatal)?;
        self.workspace = Some(CmgBatchWorkspace::try_new(pre, columns, usize::MAX).map_err(fatal)?);
        self.columns = columns;
        Ok(())
    }

    /// Lanes `first..first + cols` of the level-major `r` (row stride
    /// `width`) into the same lanes of `z`.
    fn batch(
        &mut self,
        r: &[f64],
        z: &mut [f64],
        width: usize,
        first: usize,
        cols: usize,
    ) -> Result<()> {
        let level_values = self.component.len() * cols;
        let values = self.vertices * cols;
        // Gather and centre the input on the level vertices.
        self.clear_sums(cols);
        for (level, &v) in self.vertex.iter().enumerate() {
            if v == ISOLATED {
                continue;
            }
            let v = v as usize;
            let c = self.component[v] as usize * cols;
            for k in 0..cols {
                let x = r[level * width + first + k];
                if !x.is_finite() {
                    return Err(Error::InvalidData("local solve input is not finite"));
                }
                self.rhs[v * cols + k] = x;
                neumaier(&mut self.sums[c + k], &mut self.corrections[c + k], x);
            }
        }
        self.means(cols);
        for (row, &c) in self.rhs[..level_values]
            .chunks_exact_mut(cols)
            .zip(&self.component)
        {
            let mean = &self.sums[c as usize * cols..][..cols];
            for (x, m) in row.iter_mut().zip(mean) {
                *x -= m;
            }
        }
        self.rhs[level_values..values].fill(0.0);
        let workspace = self.workspace.as_mut().expect("reserved CMG workspace");
        self.preconditioner
            .apply_compatible_batch_into(
                &self.rhs[..values],
                &mut self.solution[..values],
                cols,
                workspace,
                ApplyContext::default(),
            )
            .map_err(fatal)?;
        // Restrict to the level vertices and centre the output.
        self.clear_sums(cols);
        for (row, &c) in self.solution[..level_values]
            .chunks_exact(cols)
            .zip(&self.component)
        {
            let c = c as usize * cols;
            for (k, &x) in row.iter().enumerate() {
                neumaier(&mut self.sums[c + k], &mut self.corrections[c + k], x);
            }
        }
        self.means(cols);
        for (level, &v) in self.vertex.iter().enumerate() {
            if v == ISOLATED {
                continue;
            }
            let v = v as usize;
            let c = self.component[v] as usize * cols;
            for k in 0..cols {
                let x = self.solution[v * cols + k] - self.sums[c + k];
                if !x.is_finite() {
                    return Err(Error::InvalidData("CMG local solve output is not finite"));
                }
                z[level * width + first + k] = x;
            }
        }
        Ok(())
    }

    fn clear_sums(&mut self, cols: usize) {
        let n = self.sizes.len() * cols;
        self.sums[..n].fill(0.0);
        self.corrections[..n].fill(0.0);
    }

    /// Turn the compensated sums into component means.
    fn means(&mut self, cols: usize) {
        for (c, &size) in self.sizes.iter().enumerate() {
            let size = f64::from(size);
            for k in c * cols..(c + 1) * cols {
                self.sums[k] = (self.sums[k] + self.corrections[k]) / size;
            }
        }
    }
}

impl LocalSolve for CmgLocal {
    fn apply(&mut self, r: &[f64], z: &mut [f64], width: usize) -> Result<()> {
        let n = self.vertex.len().checked_mul(width);
        if width == 0 || n != Some(r.len()) || n != Some(z.len()) {
            return Err(Error::InvalidInput(
                "local solve dimensions are inconsistent",
            ));
        }
        self.reserve(width)?;
        let mut first = 0;
        while first < width {
            let cols = (width - first).min(MAX_COLUMNS);
            self.batch(r, z, width, first, cols)?;
            first += cols;
        }
        for (row, &v) in z.chunks_exact_mut(width).zip(&self.vertex) {
            if v == ISOLATED {
                row.fill(0.0);
            }
        }
        Ok(())
    }

    fn bytes(&self) -> u128 {
        fn held<T>(values: &Vec<T>) -> u128 {
            (values.capacity() * std::mem::size_of::<T>()) as u128
        }
        held(&self.vertex)
            + held(&self.component)
            + held(&self.sizes)
            + held(&self.rhs)
            + held(&self.solution)
            + held(&self.sums)
            + held(&self.corrections)
            + self.preconditioner.retained_bytes() as u128
            + self.workspace_bytes as u128
            + std::mem::size_of::<Self>() as u128
    }
}

enum Shape {
    /// One level: no edge.
    Single,
    Clique,
    Star,
}

/// How segment `cells` with total `total` enters the graph.
fn shape(cells: &[(u32, f64)], total: f64) -> Shape {
    match cells.len() {
        0 | 1 => Shape::Single,
        n if n > STAR_LEVELS => Shape::Star,
        _ => {
            // The smallest clique weight comes from the two lightest cells.
            let (mut lo, mut next) = (f64::INFINITY, f64::INFINITY);
            for &(_, a) in cells {
                if a < lo {
                    next = lo;
                    lo = a;
                } else if a < next {
                    next = a;
                }
            }
            if lo * (next / total) >= f64::MIN_POSITIVE {
                Shape::Clique
            } else {
                Shape::Star
            }
        }
    }
}

fn segment(block: &BlockLaplacian, s: usize) -> &[(u32, f64)] {
    &block.entries[block.offsets[s]..block.offsets[s + 1]]
}

fn validate(block: &BlockLaplacian) -> Result<()> {
    let segments = block.totals.len();
    let offsets = &block.offsets;
    if offsets.len() != segments + 1
        || offsets[0] != 0
        || offsets[segments] != block.entries.len()
        || offsets.windows(2).any(|w| w[0] > w[1])
        || block.levels >= ISOLATED as usize
    {
        return Err(Error::InvalidInput(
            "block Laplacian layout is inconsistent",
        ));
    }
    if block
        .entries
        .iter()
        .any(|&(level, a)| level as usize >= block.levels || !(a.is_finite() && a > 0.0))
    {
        return Err(Error::InvalidData("block Laplacian entry is invalid"));
    }
    if block.totals.iter().any(|w| !(w.is_finite() && *w > 0.0)) {
        return Err(Error::InvalidData(
            "block Laplacian segment total is invalid",
        ));
    }
    Ok(())
}

/// Component of every level vertex (dense labels in first-appearance order)
/// and each component's size.
fn components(
    block: &BlockLaplacian,
    vertex: &[u32],
    connected: usize,
    poll: &dyn Fn(usize) -> Result<()>,
) -> Result<(Vec<u32>, Vec<u32>)> {
    let mut parent: Vec<u32> = with_capacity(connected)?;
    parent.extend(0..connected as u32);
    fn root(parent: &mut [u32], mut v: u32) -> u32 {
        while parent[v as usize] != v {
            let up = parent[parent[v as usize] as usize];
            parent[v as usize] = up;
            v = up;
        }
        v
    }
    for s in 0..block.totals.len() {
        poll(s)?;
        let cells = segment(block, s);
        if let Some((&(first, _), rest)) = cells.split_first() {
            for &(level, _) in rest {
                let a = root(&mut parent, vertex[first as usize]);
                let b = root(&mut parent, vertex[level as usize]);
                parent[a.max(b) as usize] = a.min(b);
            }
        }
    }
    // Unions and path halving only ever point a vertex at a smaller one, so
    // in ascending order a vertex's parent already holds its label.
    let mut count = 0;
    for v in 0..connected {
        let p = parent[v] as usize;
        parent[v] = if p == v {
            count += 1;
            count - 1
        } else {
            parent[p]
        };
    }
    let mut sizes = filled(count as usize, 0_u32)?;
    for &label in &parent {
        sizes[label as usize] += 1;
    }
    Ok((parent, sizes))
}

/// Neumaier's compensated addition of `x` to `sum`.
fn neumaier(sum: &mut f64, correction: &mut f64, x: f64) {
    let t = *sum + x;
    *correction += if sum.abs() >= x.abs() {
        (*sum - t) + x
    } else {
        (x - t) + *sum
    };
    *sum = t;
}

fn fatal(error: CmgError) -> Error {
    match error {
        CmgError::Cancelled { .. } => Error::Cancelled,
        CmgError::MemoryBudgetExceeded { .. }
        | CmgError::AllocationFailed { .. }
        | CmgError::VertexIndexTooWide { .. } => {
            Error::OutOfMemory("unable to allocate the CMG local solve")
        }
        _ => Error::InvalidData("CMG local solve failed"),
    }
}

/// Construction errors: cancellation and allocation failures are fatal;
/// a numerical refusal of the graph or hierarchy declines the solve.
fn built<T>(error: CmgError) -> Result<Option<T>> {
    match error {
        CmgError::Cancelled { .. }
        | CmgError::MemoryBudgetExceeded { .. }
        | CmgError::AllocationFailed { .. }
        | CmgError::VertexIndexTooWide { .. } => Err(fatal(error)),
        CmgError::InvalidEdgeWeight { .. }
        | CmgError::NonFiniteMatrixValue { .. }
        | CmgError::NonPositivePivot { .. } => Ok(None),
        _ => Err(Error::InvalidInput("CMG local solve graph is inconsistent")),
    }
}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| Error::OutOfMemory("unable to reserve the CMG local solve"))
}

fn with_capacity<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    reserve(&mut values, count)?;
    Ok(values)
}

fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut values = with_capacity(count)?;
    values.resize(count, value);
    Ok(values)
}

/// `values` becomes `count` zeros, keeping a sufficient allocation.
fn refill(values: &mut Vec<f64>, count: usize) -> Result<()> {
    values.clear();
    reserve(values, count)?;
    values.resize(count, 0.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEVER: &(dyn Fn() -> bool + Sync) = &|| false;

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
        fn uniform(&mut self) -> f64 {
            (self.next() >> 11) as f64 / (1_u64 << 53) as f64
        }
    }

    /// A block from per-segment `(level, a_sj)` lists, `W_s = Σ_j a_sj`.
    fn block(levels: usize, segments: &[Vec<(u32, f64)>]) -> BlockLaplacian {
        let mut out = BlockLaplacian {
            levels,
            offsets: vec![0],
            ..BlockLaplacian::default()
        };
        for cells in segments {
            out.entries.extend_from_slice(cells);
            out.offsets.push(out.entries.len());
            out.totals.push(cells.iter().map(|c| c.1).sum());
        }
        out
    }

    /// `y = S_bb x` from the definition, one column.
    fn laplacian(block: &BlockLaplacian, x: &[f64], y: &mut [f64]) {
        y.fill(0.0);
        for s in 0..block.totals.len() {
            let cells = segment(block, s);
            let mean = cells.iter().map(|&(j, a)| a * x[j as usize]).sum::<f64>() / block.totals[s];
            for &(j, a) in cells {
                y[j as usize] += a * (x[j as usize] - mean);
            }
        }
    }

    fn diagonal(block: &BlockLaplacian) -> Vec<f64> {
        let mut d = vec![0.0; block.levels];
        for s in 0..block.totals.len() {
            for &(j, a) in segment(block, s) {
                d[j as usize] += a - a * a / block.totals[s];
            }
        }
        d
    }

    /// Component labels of the block's graph by breadth-first search
    /// (`None` for isolated levels).
    fn labels(block: &BlockLaplacian) -> Vec<Option<usize>> {
        let mut by_level = vec![Vec::new(); block.levels];
        for s in 0..block.totals.len() {
            let cells = segment(block, s);
            if cells.len() > 1 {
                for &(j, _) in cells {
                    by_level[j as usize].push(s);
                }
            }
        }
        let mut label = vec![None; block.levels];
        let mut count = 0;
        for start in 0..block.levels {
            if label[start].is_some() || by_level[start].is_empty() {
                continue;
            }
            label[start] = Some(count);
            let mut queue = vec![start];
            while let Some(j) = queue.pop() {
                for &s in &by_level[j] {
                    for &(k, _) in segment(block, s) {
                        if label[k as usize].is_none() {
                            label[k as usize] = Some(count);
                            queue.push(k as usize);
                        }
                    }
                }
            }
            count += 1;
        }
        label
    }

    /// Remove each component's mean (isolated levels become zero).
    fn centre(labels: &[Option<usize>], x: &mut [f64]) {
        let count = labels.iter().flatten().max().map_or(0, |m| m + 1);
        let mut sums = vec![(0.0, 0.0); count];
        for (l, &v) in labels.iter().zip(x.iter()) {
            if let Some(c) = l {
                sums[*c].0 += v;
                sums[*c].1 += 1.0;
            }
        }
        for (l, v) in labels.iter().zip(x.iter_mut()) {
            *v = l.map_or(0.0, |c| *v - sums[c].0 / sums[c].1);
        }
    }

    fn dot(a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    fn norm(a: &[f64]) -> f64 {
        dot(a, a).sqrt()
    }

    /// Two random halves joined by nothing, plus isolated levels that occur
    /// only in single-level segments or nowhere. Segment sizes span cliques,
    /// the star cutoff and wide stars.
    fn random_block(seed: u64, half: usize) -> BlockLaplacian {
        let mut rng = Rng(seed);
        let mut segments = Vec::new();
        for part in 0..2 {
            let base = part * half;
            for _ in 0..3 * half / 2 {
                let size = match rng.below(10) {
                    0..=1 => 1,
                    2..=4 => 2,
                    5..=6 => 3,
                    7 => 4 + rng.below(2),
                    _ => 6 + rng.below(7),
                };
                let mut cells: Vec<(u32, f64)> = Vec::new();
                while cells.len() < size {
                    let level = (base + rng.below(half)) as u32;
                    if cells.iter().all(|c| c.0 != level) {
                        cells.push((level, 0.05 + rng.uniform()));
                    }
                }
                segments.push(cells);
            }
        }
        for j in 0..10 {
            segments.push(vec![((2 * half + j) as u32, 0.5)]);
        }
        block(2 * half + 20, &segments)
    }

    fn apply(m: &mut CmgLocal, r: &[f64], width: usize) -> Vec<f64> {
        let mut z = vec![f64::NAN; r.len()];
        m.apply(r, &mut z, width).unwrap();
        z
    }

    fn column(x: &[f64], width: usize, c: usize) -> Vec<f64> {
        x.iter().skip(c).step_by(width).copied().collect()
    }

    #[test]
    fn random_blocks_give_fixed_symmetric_semidefinite_actions() {
        for seed in [7, 11] {
            let b = random_block(seed, 600);
            let n = b.levels;
            let labels = labels(&b);
            let mut m = CmgLocal::build(&b, NEVER).unwrap().expect("solve");
            assert!(m.preconditioner.hierarchy().levels().len() > 1);
            let mut rng = Rng(seed + 100);
            let width = 6;
            let r: Vec<f64> = (0..n * width).map(|_| rng.uniform() - 0.3).collect();
            let z = apply(&mut m, &r, width);
            let bytes = m.bytes();
            // Fixed: repeated and per-column applications give the same bits.
            assert_eq!(apply(&mut m, &r, width), z);
            for c in 0..width {
                let alone = apply(&mut m, &column(&r, width, c), 1);
                assert_eq!(alone, column(&z, width, c), "column {c}");
            }
            assert_eq!(m.bytes(), bytes);
            for i in 0..width {
                let (u, mu) = (column(&r, width, i), column(&z, width, i));
                let scale = norm(&u) * norm(&mu);
                // Positive on the range, centred per component, zero on
                // isolated levels.
                assert!(dot(&u, &mu) > 1e-3 * scale);
                let mut centred = mu.clone();
                centre(&labels, &mut centred);
                for (a, b) in centred.iter().zip(&mu) {
                    assert!((a - b).abs() <= 1e-12 * norm(&mu));
                }
                for (l, v) in labels.iter().zip(&mu) {
                    if l.is_none() {
                        assert_eq!(*v, 0.0);
                    }
                }
                for j in 0..i {
                    let (v, mv) = (column(&r, width, j), column(&z, width, j));
                    let gap = (dot(&u, &mv) - dot(&v, &mu)).abs();
                    assert!(gap <= 1e-12 * scale, "symmetry gap {gap} at scale {scale}");
                }
            }
            // Integer constants per component (and anything on isolated
            // levels) are centred exactly and map to exact zeros.
            let constants: Vec<f64> = labels
                .iter()
                .enumerate()
                .flat_map(|(j, l)| {
                    let v = l.map_or(j as f64 * 0.37, |c| (c % 7) as f64 - 3.0);
                    [v, 2.0 * v]
                })
                .collect();
            assert!(apply(&mut m, &constants, 2).iter().all(|&v| v == 0.0));
        }
    }

    #[test]
    fn direct_terminal_is_the_exact_pseudo_inverse() {
        // Below CMG's direct threshold the hierarchy is one exact factor, so
        // S (M r) must equal the centred r: this checks the clique weights,
        // the auxiliary stars (sizes 6 and 9) and an underflowing clique that
        // becomes a star.
        let mut rng = Rng(3);
        let n = 120;
        let mut segments: Vec<Vec<(u32, f64)>> = (0..n - 1)
            .map(|j| vec![(j as u32, 0.5 + rng.uniform()), (j as u32 + 1, 1.0)])
            .collect();
        for size in [3, 5, 6, 9] {
            let start = rng.below(n - size);
            segments.push(
                (start..start + size)
                    .map(|j| (j as u32, 0.1 + rng.uniform()))
                    .collect(),
            );
        }
        segments.push(vec![(10, 1e-170), (11, 1e-170), (12, 1.0)]);
        let total = |cells: &[(u32, f64)]| cells.iter().map(|c| c.1).sum::<f64>();
        let five = &segments[n];
        assert!(five.len() == 5 && matches!(shape(five, total(five)), Shape::Clique));
        let tiny = segments.last().unwrap();
        assert!(matches!(shape(tiny, total(tiny)), Shape::Star));
        let b = block(n + 5, &segments);
        let labels = labels(&b);
        let mut m = CmgLocal::build(&b, NEVER).unwrap().expect("solve");
        assert_eq!(m.vertices, n + 3);
        assert!(m.preconditioner.terminal_factor().is_some());
        for _ in 0..3 {
            let r: Vec<f64> = (0..n + 5).map(|_| rng.uniform() - 0.5).collect();
            let z = apply(&mut m, &r, 1);
            let mut sz = vec![0.0; n + 5];
            laplacian(&b, &z, &mut sz);
            let mut expected = r.clone();
            centre(&labels, &mut expected);
            let error: Vec<f64> = sz.iter().zip(&expected).map(|(a, b)| a - b).collect();
            assert!(norm(&error) <= 1e-9 * norm(&expected), "{}", norm(&error));
        }
    }

    /// Plain PCG on `S_bb x = b` to `||r|| <= 1e-8 ||b||`; the iteration count.
    fn pcg(
        block: &BlockLaplacian,
        b: &[f64],
        precondition: &mut dyn FnMut(&[f64], &mut [f64]),
    ) -> usize {
        let n = b.len();
        let (mut x, mut r, mut z, mut q) = (vec![0.0; n], b.to_vec(), vec![0.0; n], vec![0.0; n]);
        precondition(&r, &mut z);
        let mut p = z.clone();
        let mut rz = dot(&r, &z);
        let target = 1e-8 * norm(b);
        for iteration in 1..=50_000 {
            laplacian(block, &p, &mut q);
            let alpha = rz / dot(&p, &q);
            for i in 0..n {
                x[i] += alpha * p[i];
                r[i] -= alpha * q[i];
            }
            if norm(&r) <= target {
                laplacian(block, &x, &mut q);
                let true_residual: Vec<f64> = b.iter().zip(&q).map(|(a, b)| a - b).collect();
                assert!(norm(&true_residual) <= 1e-6 * norm(b));
                return iteration;
            }
            precondition(&r, &mut z);
            let next = dot(&r, &z);
            for i in 0..n {
                p[i] = z[i] + next / rz * p[i];
            }
            rz = next;
        }
        panic!("PCG did not converge");
    }

    /// Low mobility: clusters of six firms linked internally by movers
    /// (pairs, a triple, a six-firm star), consecutive clusters linked in a
    /// long chain by one light mover each, plus stayers.
    fn chain(clusters: usize) -> BlockLaplacian {
        let m = 6;
        let mut rng = Rng(5);
        let mut segments = Vec::new();
        for c in 0..clusters {
            let f = |i: usize| (c * m + i % m) as u32;
            for i in 0..m {
                segments.push(vec![(f(i), 1.0 + rng.uniform()), (f(i + 1), 1.0)]);
                segments.push(vec![(f(i), 3.0)]);
            }
            segments.push(vec![(f(0), 1.0), (f(2), 0.5), (f(4), 0.5)]);
            segments.push((0..m).map(|i| (f(i), 0.2)).collect());
            if c + 1 < clusters {
                segments.push(vec![(f(m - 1), 1.0), ((c * m + m) as u32, 0.02)]);
            }
        }
        block(clusters * m, &segments)
    }

    #[test]
    fn cmg_needs_far_fewer_pcg_iterations_than_jacobi_on_a_chain() {
        let b = chain(400);
        let labels = labels(&b);
        assert!(labels.iter().all(|l| *l == Some(0)));
        let mut rng = Rng(9);
        let mut rhs: Vec<f64> = (0..b.levels).map(|_| rng.uniform()).collect();
        centre(&labels, &mut rhs);
        let d = diagonal(&b);
        let jacobi = pcg(&b, &rhs, &mut |r, z| {
            for i in 0..r.len() {
                z[i] = r[i] / d[i];
            }
        });
        let mut m = CmgLocal::build(&b, NEVER).unwrap().expect("solve");
        assert!(m.preconditioner.hierarchy().levels().len() > 1);
        let cmg = pcg(&b, &rhs, &mut |r, z| m.apply(r, z, 1).unwrap());
        eprintln!(
            "chain of {} levels: Jacobi PCG {jacobi} iterations, CMG PCG {cmg}",
            b.levels
        );
        assert!(10 * cmg < jacobi, "Jacobi {jacobi}, CMG {cmg}");
    }

    #[test]
    fn degenerate_blocks_decline_and_invalid_blocks_fail() {
        let singles = block(4, &[vec![(0, 1.0)], vec![(3, 2.0)]]);
        assert!(CmgLocal::build(&singles, NEVER).unwrap().is_none());
        assert!(CmgLocal::build(&block(3, &[]), NEVER).unwrap().is_none());
        let mut bad = block(3, &[vec![(0, 1.0), (1, 1.0)]]);
        bad.entries[1].1 = -1.0;
        assert!(matches!(
            CmgLocal::build(&bad, NEVER),
            Err(Error::InvalidData(_))
        ));
        bad.entries[1] = (3, 1.0);
        assert!(matches!(
            CmgLocal::build(&bad, NEVER),
            Err(Error::InvalidData(_))
        ));
        bad.entries[1] = (1, 1.0);
        bad.offsets[1] = 3;
        assert!(matches!(
            CmgLocal::build(&bad, NEVER),
            Err(Error::InvalidInput(_))
        ));
        let cancelled: &(dyn Fn() -> bool + Sync) = &|| true;
        assert!(matches!(
            CmgLocal::build(&chain(4), cancelled),
            Err(Error::Cancelled)
        ));
        let mut m = CmgLocal::build(&chain(4), NEVER).unwrap().unwrap();
        let mut z = vec![0.0; 24];
        assert!(matches!(
            m.apply(&[0.0; 24], &mut z, 2),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            m.apply(&[0.0; 24], &mut z, 0),
            Err(Error::InvalidInput(_))
        ));
    }
}
