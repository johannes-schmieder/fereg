use cmg::Laplacian;
use std::collections::{BTreeMap, VecDeque};

/// Tree-plus-diagonal LDL with stable nonnegative grounding accumulation.
pub struct Forest {
    parent: Vec<usize>,
    weight: Vec<f64>,
    pivot: Vec<f64>,
    order: Vec<usize>,
}
impl Forest {
    pub fn new(graph: &Laplacian) -> Self {
        let (heavy, _) = cmg::maximum_weight_forest(graph);
        let mut roots: Vec<_> = (0..graph.vertex_count()).collect();
        let mut selected = vec![false; graph.edge_count()];
        fn root(roots: &mut [usize], mut x: usize) -> usize {
            while roots[x] != x {
                roots[x] = roots[roots[x]];
                x = roots[x];
            }
            x
        }
        for (i, e) in graph.edges().iter().enumerate() {
            if heavy[e.u()] == e.v() || heavy[e.v()] == e.u() {
                let a = root(&mut roots, e.u());
                let b = root(&mut roots, e.v());
                if a != b {
                    roots[a] = b;
                    selected[i] = true;
                }
            }
        }
        Self::with_edges(graph, &selected)
    }
    pub fn with_edges(graph: &Laplacian, selected: &[bool]) -> Self {
        let n = graph.vertex_count();
        let mut adjacency = vec![vec![]; n];
        let mut grounding = vec![0.; n];
        for (e, &keep) in graph.edges().iter().zip(selected) {
            if keep {
                adjacency[e.u()].push((e.v(), e.weight()));
                adjacency[e.v()].push((e.u(), e.weight()));
            } else {
                grounding[e.u()] += 2. * e.weight();
                grounding[e.v()] += 2. * e.weight();
            }
        }
        let mut parent = vec![usize::MAX; n];
        let mut weight = vec![0.; n];
        let mut order = Vec::with_capacity(n);
        for start in 0..n {
            if parent[start] != usize::MAX {
                continue;
            }
            parent[start] = start;
            let mut stack = vec![start];
            while let Some(v) = stack.pop() {
                order.push(v);
                for &(u, w) in &adjacency[v] {
                    if u == parent[v] {
                        continue;
                    }
                    assert_eq!(parent[u], usize::MAX, "selected edges must be acyclic");
                    parent[u] = v;
                    weight[u] = w;
                    stack.push(u);
                }
            }
        }
        let mut pivot = vec![0.; n];
        for &v in order.iter().rev() {
            pivot[v] = grounding[v] + weight[v];
            if parent[v] != v {
                grounding[parent[v]] += weight[v] * (grounding[v] / pivot[v]);
            }
        }
        Self {
            parent,
            weight,
            pivot,
            order,
        }
    }
    pub fn apply(&self, rhs: &[f64], out: &mut [f64]) {
        out.copy_from_slice(rhs);
        for &v in self.order.iter().rev() {
            if self.parent[v] != v {
                out[self.parent[v]] += self.weight[v] / self.pivot[v] * out[v];
            }
        }
        for &v in &self.order {
            if self.parent[v] == v {
                out[v] = if self.pivot[v] > 0. {
                    out[v] / self.pivot[v]
                } else {
                    0.
                };
            } else {
                out[v] = (out[v] + self.weight[v] * out[self.parent[v]]) / self.pivot[v];
            }
        }
    }
}

pub struct Step {
    vertex: usize,
    neighbors: Vec<(usize, f64)>,
    pivot: f64,
}
pub struct Elimination {
    steps: Vec<Step>,
    pub core: Laplacian,
    kept: Vec<usize>,
    core_rhs: Vec<f64>,
    core_x: Vec<f64>,
}
impl Elimination {
    pub fn new(graph: &Laplacian) -> Self {
        let n = graph.vertex_count();
        let mut adj = vec![BTreeMap::new(); n];
        for e in graph.edges() {
            adj[e.u()].insert(e.v(), e.weight());
            adj[e.v()].insert(e.u(), e.weight());
        }
        let mut queue: VecDeque<_> = (0..n).filter(|&i| adj[i].len() <= 2).collect();
        let mut removed = vec![false; n];
        let mut steps = vec![];
        while let Some(v) = queue.pop_front() {
            if removed[v] || adj[v].is_empty() || adj[v].len() > 2 {
                continue;
            }
            let neighbors: Vec<_> = adj[v].iter().map(|(&u, &w)| (u, w)).collect();
            let pivot: f64 = neighbors.iter().map(|x| x.1).sum();
            for &(u, _) in &neighbors {
                adj[u].remove(&v);
            }
            if neighbors.len() == 2 {
                let (a, wa) = neighbors[0];
                let (b, wb) = neighbors[1];
                let fill = wa * (wb / pivot);
                *adj[a].entry(b).or_insert(0.) += fill;
                *adj[b].entry(a).or_insert(0.) += fill;
            }
            removed[v] = true;
            adj[v].clear();
            for &(u, _) in &neighbors {
                if adj[u].len() <= 2 {
                    queue.push_back(u);
                }
            }
            steps.push(Step {
                vertex: v,
                neighbors,
                pivot,
            });
        }
        let kept: Vec<_> = (0..n).filter(|&i| !removed[i]).collect();
        let mut mapping = vec![usize::MAX; n];
        for (i, &v) in kept.iter().enumerate() {
            mapping[v] = i;
        }
        let mut edges = vec![];
        for &v in &kept {
            for (&u, &w) in &adj[v] {
                if v < u {
                    edges.push((mapping[v], mapping[u], w));
                }
            }
        }
        let core = Laplacian::from_edges(kept.len(), edges).unwrap();
        let core_rhs = vec![0.; kept.len()];
        let core_x = core_rhs.clone();
        Self {
            steps,
            core,
            kept,
            core_rhs,
            core_x,
        }
    }
    pub fn apply(&mut self, rhs: &[f64], out: &mut [f64]) {
        self.apply_with(rhs, out, |graph, b, x| {
            for ((v, &r), &d) in x.iter_mut().zip(b).zip(graph.diagonal()) {
                *v = if d > 0. { 0.5 * r / d } else { 0. };
            }
        });
    }
    pub fn apply_with(
        &mut self,
        rhs: &[f64],
        out: &mut [f64],
        mut core_solve: impl FnMut(&Laplacian, &[f64], &mut [f64]),
    ) {
        out.copy_from_slice(rhs);
        for step in &self.steps {
            let value = out[step.vertex] / step.pivot;
            for &(u, w) in &step.neighbors {
                out[u] += w * value;
            }
        }
        for (b, &v) in self.core_rhs.iter_mut().zip(&self.kept) {
            *b = out[v];
        }
        core_solve(&self.core, &self.core_rhs, &mut self.core_x);
        for (&v, &x) in self.kept.iter().zip(&self.core_x) {
            out[v] = x;
        }
        for step in self.steps.iter().rev() {
            let rest: f64 = step.neighbors.iter().map(|&(u, w)| w * out[u]).sum();
            out[step.vertex] = (out[step.vertex] + rest) / step.pivot;
        }
    }
    pub fn remaining(&self) -> usize {
        self.kept.len()
    }
}
